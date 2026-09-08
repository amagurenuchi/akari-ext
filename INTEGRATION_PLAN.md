# Fraction migration: sequential integration plan

This guide rebuilds the `i64 / NonZeroU64` fraction change from the original `i64 / i64` implementation. Apply the stages in order. Every stage leaves the crate compilable.

The patches assume you start from the original files before this migration. If the migration is already present, use the patches as a reading guide rather than applying them again.

## How to use each patch

For each stage:

1. Copy only the contents of its `diff` block into a temporary patch file.
2. Check it without changing files: `git apply --check stage-N.patch`.
3. Apply it: `git apply stage-N.patch`.
4. Run the verification command shown after the patch.
5. Commit that stage before continuing if you want a clean, reviewable history.

You can also apply the `+` and `-` lines manually. Lines beginning with `-` are removed; lines beginning with `+` are added.

## Why Stage 1 is intentionally atomic

Changing `denom` from `i64` to `NonZeroU64` immediately affects construction, arithmetic, ordering, formatting, remainder, and `Default`. Updating only one of those areas would produce type errors. For that reason, the first stage changes all production-code consumers together. Later stages add focused tests one behavior at a time.

## Stage 1: Migrate the representation and production code

### Easy explanation

This stage stores the sign only in the numerator and makes a zero denominator impossible after construction. It converts signed inputs into unsigned magnitudes, reduces them, then safely reconstructs the signed numerator.

Normal arithmetic is routed through checked arithmetic. Large temporary values use `u128`, allowing an intermediate result to be reduced before deciding whether it fits the final `i64 / NonZeroU64` representation.

### Technical details

- `NonZeroU64` establishes the denominator invariant at the type level.
- `unsigned_abs()` handles `i64::MIN`; `abs()` cannot.
- XOR combines the input signs: `(numer < 0) ^ (denom < 0)`.
- `from_magnitudes` is the single normalization boundary used by construction and arithmetic.
- Reduction occurs before the `i64` and `u64` range checks.
- Addition and subtraction use sign-and-magnitude arithmetic so two large cross-products never overflow signed `i128` merely while being added.
- Regular operators panic consistently when checked operations return `None`, instead of wrapping only in release builds.

### Patch

```diff
diff --git a/akari/src/fraction/definition.rs b/akari/src/fraction/definition.rs
--- a/akari/src/fraction/definition.rs
+++ b/akari/src/fraction/definition.rs
@@ -1,9 +1,11 @@
+use core::num::NonZeroU64;
+
 /// Represents an exact rational fraction `numerator / denominator`.
 ///
 /// Under the hood, fractions are kept reduced to lowest terms, with the denominator
-/// guaranteed to be strictly positive (`> 0`).
+/// guaranteed by its type to be strictly positive (`> 0`).
 #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
 pub struct Fraction {
     pub(crate) numer: i64,
-    pub(crate) denom: i64,
+    pub(crate) denom: NonZeroU64,
 }
diff --git a/akari/src/fraction/gcd.rs b/akari/src/fraction/gcd.rs
--- a/akari/src/fraction/gcd.rs
+++ b/akari/src/fraction/gcd.rs
@@ -1,7 +1,15 @@
 /// Helper function to compute Greatest Common Divisor (GCD) using Euclidean algorithm.
-pub(crate) fn gcd(mut a: i64, mut b: i64) -> i64 {
-    a = a.abs();
-    b = b.abs();
+pub(crate) fn gcd(mut a: u64, mut b: u64) -> u64 {
+    while b != 0 {
+        let t = b;
+        b = a % b;
+        a = t;
+    }
+    if a == 0 { 1 } else { a }
+}
+
+/// `u128` variant used for arithmetic intermediates.
+pub(crate) fn gcd_u128(mut a: u128, mut b: u128) -> u128 {
     while b != 0 {
         let t = b;
         b = a % b;
diff --git a/akari/src/fraction/fraction.rs b/akari/src/fraction/fraction.rs
--- a/akari/src/fraction/fraction.rs
+++ b/akari/src/fraction/fraction.rs
@@ -1,5 +1,7 @@
+use core::num::NonZeroU64;
+
 use crate::fraction::definition::Fraction;
-use crate::fraction::gcd::gcd;
+use crate::fraction::gcd::{gcd, gcd_u128};
 
 impl Fraction {
     /// Default denominator cap for continued-fraction approximation.
@@ -9,32 +11,48 @@ impl Fraction {
     pub const DEFAULT_MAX_DENOM: i64 = 1_000_000;
 
     /// Creates a new `Fraction` reduced to lowest terms.
-    /// The new function already calls try_new, so it is reduced to its simplest terms from the start.
-    /// By design, the denominator should not be zero. 
-    /// Whatever the end user assigns to the denominator that causes it to be zero is a problem on their side.
-    /// The Checked functions for arithmetic funtions are available for further validation, but might be removed if this zero check is sufficient.
+    ///
+    /// A negative denominator is normalized by moving its sign to the numerator.
+    /// Panics when the denominator is zero or the normalized numerator cannot be
+    /// represented by `i64` (the latter is only possible for `i64::MIN / -1`).
     pub fn new(numer: i64, denom: i64) -> Self {
-        Self::try_new(numer, denom).expect("Fraction::new requires a non-zero denominator")
+        Self::try_new(numer, denom)
+            .expect("Fraction::new requires a non-zero denominator and representable numerator")
     }
 
-    /// Fallible constructor that returns `None` when the denominator is zero.
+    /// Fallible constructor that returns `None` when the denominator is zero or
+    /// normalization would produce a numerator outside the `i64` range.
     pub fn try_new(numer: i64, denom: i64) -> Option<Self> {
         if denom == 0 {
             return None;
         }
-        let g = gcd(numer, denom);
-        let mut n = numer / g;
-        let mut d = denom / g;
-        if d < 0 {
-            n = -n;
-            d = -d;
-        }
-        Some(Self { numer: n, denom: d })
+
+        Self::from_magnitudes(
+            numer.unsigned_abs() as u128,
+            denom.unsigned_abs() as u128,
+            (numer < 0) ^ (denom < 0),
+        )
+    }
+
+    /// Creates a reduced fraction from a numerator and an already-positive denominator.
+    ///
+    /// Unlike [`new`](Self::new), this accepts denominators through the full `u64`
+    /// range and cannot fail due to a zero or negative denominator.
+    pub fn new_nonzero(numer: i64, denom: NonZeroU64) -> Self {
+        let g = gcd(numer.unsigned_abs(), denom.get());
+        let numer_magnitude = numer.unsigned_abs() / g;
+        let denom = NonZeroU64::new(denom.get() / g).expect("a reduced denominator is non-zero");
+        let numer = Self::signed_numerator(numer_magnitude as u128, numer < 0)
+            .expect("reducing an i64 numerator remains representable");
+        Self { numer, denom }
     }
 
     /// Creates a fraction from an integer `n / 1`.
     pub fn from_integer(n: i64) -> Self {
-        Self { numer: n, denom: 1 }
+        Self {
+            numer: n,
+            denom: NonZeroU64::new(1).expect("one is non-zero"),
+        }
     }
 
     /// Returns the numerator of the fraction.
@@ -43,20 +61,25 @@ impl Fraction {
     }
 
     /// Returns the denominator of the fraction (guaranteed > 0).
-    pub fn denom(&self) -> i64 {
+    pub fn denom(&self) -> u64 {
+        self.denom.get()
+    }
+
+    /// Returns the denominator with its non-zero invariant preserved in the type.
+    pub fn denom_nonzero(&self) -> NonZeroU64 {
         self.denom
     }
 
     /// Converts the fraction to an `f64` representation.
     /// Technically it reads as one f64 number after conversion silently anyways.
     pub fn to_f64(&self) -> f64 {
-        self.numer as f64 / self.denom as f64
+        self.numer as f64 / self.denom.get() as f64
     }
 
     /// Returns true if the fraction is an integer (denominator is 1).
     /// This runs after the fraction is simplfied.
     pub fn is_integer(&self) -> bool {
-        self.denom == 1
+        self.denom.get() == 1
     }
 
     /// Converts an `f64` to the best rational approximation with denominator ≤ `max_denom`,
@@ -79,7 +102,7 @@ impl Fraction {
     /// assert_eq!(Fraction::from_f64_bounded(f64::NAN, 100), None);
     /// ```
     pub fn from_f64_bounded(f: f64, max_denom: i64) -> Option<Self> {
-        if !f.is_finite() {
+        if !f.is_finite() || max_denom <= 0 {
             return None;
         }
         let sign: i64 = if f < 0.0 { -1 } else { 1 };
@@ -183,4 +206,37 @@ impl Fraction {
     pub fn approx_f64(f: f64) -> Self {
         Self::from_f64(f).unwrap_or_else(|| Self::from_integer(f as i64))
     }
+
+    pub(crate) fn from_magnitudes(
+        mut numer: u128,
+        mut denom: u128,
+        negative: bool,
+    ) -> Option<Self> {
+        if denom == 0 {
+            return None;
+        }
+        if numer == 0 {
+            return Some(Self::from_integer(0));
+        }
+
+        let g = gcd_u128(numer, denom);
+        numer /= g;
+        denom /= g;
+
+        let numer = Self::signed_numerator(numer, negative)?;
+        let denom = NonZeroU64::new(u64::try_from(denom).ok()?)?;
+        Some(Self { numer, denom })
+    }
+
+    fn signed_numerator(magnitude: u128, negative: bool) -> Option<i64> {
+        if negative {
+            if magnitude == 1_u128 << 63 {
+                Some(i64::MIN)
+            } else {
+                i64::try_from(magnitude).ok()?.checked_neg()
+            }
+        } else {
+            i64::try_from(magnitude).ok()
+        }
+    }
 }
```

The next part of Stage 1 updates checked and regular operators. It belongs to the same compilation boundary, so apply both Stage 1 diff blocks before running Cargo.

```diff
diff --git a/akari/src/fraction/checked.rs b/akari/src/fraction/checked.rs
--- a/akari/src/fraction/checked.rs
+++ b/akari/src/fraction/checked.rs
@@ -3,29 +3,19 @@ use crate::fraction::definition::Fraction;
 impl Fraction {
     /// Checked addition. Returns `None` on overflow or divide-by-zero.
     pub fn checked_add(self, rhs: Self) -> Option<Self> {
-        let n = self
-            .numer
-            .checked_mul(rhs.denom)?
-            .checked_add(rhs.numer.checked_mul(self.denom)?)?;
-        let d = self.denom.checked_mul(rhs.denom)?;
-        Some(Self::new(n, d))
+        self.checked_add_with_rhs_sign(rhs, false)
     }
 
     /// Checked subtraction.
     pub fn checked_sub(self, rhs: Self) -> Option<Self> {
-        let n = self
-            .numer
-            .checked_mul(rhs.denom)?
-            .checked_sub(rhs.numer.checked_mul(self.denom)?)?;
-        let d = self.denom.checked_mul(rhs.denom)?;
-        Some(Self::new(n, d))
+        self.checked_add_with_rhs_sign(rhs, true)
     }
 
     /// Checked multiplication.
     pub fn checked_mul(self, rhs: Self) -> Option<Self> {
-        let n = self.numer.checked_mul(rhs.numer)?;
-        let d = self.denom.checked_mul(rhs.denom)?;
-        Some(Self::new(n, d))
+        let n = (self.numer.unsigned_abs() as u128) * (rhs.numer.unsigned_abs() as u128);
+        let d = (self.denom.get() as u128) * (rhs.denom.get() as u128);
+        Self::from_magnitudes(n, d, (self.numer < 0) ^ (rhs.numer < 0))
     }
 
     /// Checked division. Returns `None` if rhs numerator is 0.
@@ -33,8 +23,26 @@ impl Fraction {
         if rhs.numer == 0 {
             return None;
         }
-        let n = self.numer.checked_mul(rhs.denom)?;
-        let d = self.denom.checked_mul(rhs.numer)?;
-        Some(Self::new(n, d))
+        let n = (self.numer.unsigned_abs() as u128) * (rhs.denom.get() as u128);
+        let d = (self.denom.get() as u128) * (rhs.numer.unsigned_abs() as u128);
+        Self::from_magnitudes(n, d, (self.numer < 0) ^ (rhs.numer < 0))
+    }
+
+    fn checked_add_with_rhs_sign(self, rhs: Self, negate_rhs: bool) -> Option<Self> {
+        let lhs = (self.numer.unsigned_abs() as u128) * (rhs.denom.get() as u128);
+        let rhs_magnitude = (rhs.numer.unsigned_abs() as u128) * (self.denom.get() as u128);
+        let lhs_negative = self.numer < 0;
+        let rhs_negative = (rhs.numer < 0) ^ negate_rhs;
+
+        let (numer, negative) = if lhs_negative == rhs_negative {
+            (lhs.checked_add(rhs_magnitude)?, lhs_negative)
+        } else if lhs >= rhs_magnitude {
+            (lhs - rhs_magnitude, lhs_negative)
+        } else {
+            (rhs_magnitude - lhs, rhs_negative)
+        };
+
+        let denom = (self.denom.get() as u128) * (rhs.denom.get() as u128);
+        Self::from_magnitudes(numer, denom, negative)
     }
 }
diff --git a/akari/src/fraction/ops.rs b/akari/src/fraction/ops.rs
--- a/akari/src/fraction/ops.rs
+++ b/akari/src/fraction/ops.rs
@@ -1,12 +1,16 @@
+use crate::fraction::definition::Fraction;
 use core::fmt;
+use core::num::NonZeroU64;
 use core::ops::{
     Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign, Sub, SubAssign,
 };
-use crate::fraction::definition::Fraction;
 
 impl Default for Fraction {
     fn default() -> Self {
-        Self { numer: 0, denom: 1 }
+        Self {
+            numer: 0,
+            denom: NonZeroU64::new(1).expect("one is non-zero"),
+        }
     }
 }
 
@@ -18,18 +22,18 @@ impl PartialOrd for Fraction {
 
 impl Ord for Fraction {
     fn cmp(&self, other: &Self) -> core::cmp::Ordering {
-        let lhs = self.numer as i128 * other.denom as i128;
-        let rhs = other.numer as i128 * self.denom as i128;
+        let lhs = self.numer as i128 * other.denom.get() as i128;
+        let rhs = other.numer as i128 * self.denom.get() as i128;
         lhs.cmp(&rhs)
     }
 }
 
 impl fmt::Display for Fraction {
     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
-        if self.denom == 1 {
+        if self.denom.get() == 1 {
             write!(f, "{}", self.numer)
         } else {
-            write!(f, "{}/{}", self.numer, self.denom)
+            write!(f, "{}/{}", self.numer, self.denom.get())
         }
     }
 }
@@ -37,34 +41,32 @@ impl fmt::Display for Fraction {
 impl Add for Fraction {
     type Output = Self;
     fn add(self, rhs: Self) -> Self::Output {
-        Self::new(
-            self.numer * rhs.denom + rhs.numer * self.denom,
-            self.denom * rhs.denom,
-        )
+        self.checked_add(rhs)
+            .expect("fraction addition overflowed its representation")
     }
 }
 
 impl Sub for Fraction {
     type Output = Self;
     fn sub(self, rhs: Self) -> Self::Output {
-        Self::new(
-            self.numer * rhs.denom - rhs.numer * self.denom,
-            self.denom * rhs.denom,
-        )
+        self.checked_sub(rhs)
+            .expect("fraction subtraction overflowed its representation")
     }
 }
 
 impl Mul for Fraction {
     type Output = Self;
     fn mul(self, rhs: Self) -> Self::Output {
-        Self::new(self.numer * rhs.numer, self.denom * rhs.denom)
+        self.checked_mul(rhs)
+            .expect("fraction multiplication overflowed its representation")
     }
 }
 
 impl Div for Fraction {
     type Output = Self;
     fn div(self, rhs: Self) -> Self::Output {
-        Self::new(self.numer * rhs.denom, self.denom * rhs.numer)
+        self.checked_div(rhs)
+            .expect("fraction division by zero or overflow")
     }
 }
 
@@ -72,7 +74,7 @@ impl Rem for Fraction {
     type Output = Self;
     fn rem(self, rhs: Self) -> Self::Output {
         let div = self / rhs;
-        let int_part = div.numer / div.denom;
+        let int_part = (div.numer as i128 / div.denom.get() as i128) as i64;
         self - (rhs * Fraction::from_integer(int_part))
     }
 }
@@ -81,7 +83,10 @@ impl Neg for Fraction {
     type Output = Self;
     fn neg(self) -> Self::Output {
         Self {
-            numer: -self.numer,
+            numer: self
+                .numer
+                .checked_neg()
+                .expect("fraction negation overflowed its numerator"),
             denom: self.denom,
         }
     }
@@ -134,3 +139,9 @@ impl From<(i64, i64)> for Fraction {
         Self::new(numer, denom)
     }
 }
+
+impl From<(i64, NonZeroU64)> for Fraction {
+    fn from((numer, denom): (i64, NonZeroU64)) -> Self {
+        Self::new_nonzero(numer, denom)
+    }
+}
```

### Verify Stage 1

```bash
cargo check -p akari
cargo test -p akari fraction::
cargo check -p akari --no-default-features --features no_std
```

At this point the original fraction tests compile and pass. The new edge cases are added in the following stages so a failure can be tied to one behavior.

## Stage 2: Test construction and the public denominator API

### Easy explanation

These tests prove that the new representation handles values that broke the old signed-denominator design. They also demonstrate how callers use `NonZeroU64` directly.

### Technical details

- `i64::MIN` has magnitude `2^63`; it must be handled without signed `abs()`.
- `1 / i64::MIN` normalizes to `-1 / 2^63`.
- `i64::MIN / -1` would normalize to positive `2^63`, which does not fit `i64`, so the fallible constructor returns `None`.
- `new_nonzero` proves that stored denominators can use the complete `u64` range.
- The tuple test verifies the new `From<(i64, NonZeroU64)>` implementation.

### Patch

```diff
diff --git a/akari/src/fraction/fraction_basics.rs b/akari/src/fraction/fraction_basics.rs
--- a/akari/src/fraction/fraction_basics.rs
+++ b/akari/src/fraction/fraction_basics.rs
@@ -15,6 +15,7 @@
 /// Run this with:
 ///   cargo test --test fraction_basics
 use akari::Fraction;
+use core::num::NonZeroU64;
 
 // ─────────────────────────────────────────────────────────────────
 // 1. CONSTRUCTION: How to create a Fraction
@@ -63,6 +64,33 @@ fn test_fraction_zero_denominator() {
     assert_eq!(Fraction::try_new(5, 2), Some(Fraction::new(5, 2)));
 }
 
+/// The representation handles the complete signed-input range without calling
+/// `i64::abs`, and exposes the positive denominator as an unsigned value.
+#[test]
+fn test_fraction_integer_boundaries_and_nonzero_denominator() {
+    let min_numerator = Fraction::new(i64::MIN, 1);
+    assert_eq!(min_numerator.numer(), i64::MIN);
+    assert_eq!(min_numerator.denom(), 1);
+
+    let min_denominator = Fraction::new(1, i64::MIN);
+    assert_eq!(min_denominator.numer(), -1);
+    assert_eq!(min_denominator.denom(), 1_u64 << 63);
+    assert_eq!(
+        min_denominator.denom_nonzero(),
+        NonZeroU64::new(1_u64 << 63).unwrap()
+    );
+
+    // Normalizing i64::MIN / -1 would require the unavailable positive i64 value 2^63.
+    assert_eq!(Fraction::try_new(i64::MIN, -1), None);
+
+    let full_width_denom = Fraction::new_nonzero(1, NonZeroU64::new(u64::MAX).unwrap());
+    assert_eq!(full_width_denom.numer(), 1);
+    assert_eq!(full_width_denom.denom(), u64::MAX);
+
+    let from_tuple: Fraction = (6, NonZeroU64::new(8).unwrap()).into();
+    assert_eq!(from_tuple, Fraction::new(3, 4));
+}
+
 // ─────────────────────────────────────────────────────────────────
 // 2. ARITHMETIC: Exact fraction math
 // ─────────────────────────────────────────────────────────────────
```

### Verify Stage 2

```bash
cargo test -p akari fraction::fraction_basics::test_fraction_integer_boundaries_and_nonzero_denominator
```

## Stage 3: Test wide arithmetic and final range checks

### Easy explanation

The first test multiplies fractions whose direct `i64` products would overflow even though the reduced answer is simply `1`. The second test confirms that a genuinely unrepresentable result returns `None`.

### Technical details

For `(i64::MAX / 2) * (2 / i64::MAX)`, computing in `i64` overflows before reduction. Computing magnitudes in `u128` produces the exact intermediate rational, and `from_magnitudes` reduces it to `1/1` before converting back to the stored widths.

By contrast, `i64::MAX + 1` reduces to positive `2^63 / 1`, which cannot fit the signed numerator. `checked_add` must therefore return `None`.

### Patch

```diff
diff --git a/akari/src/fraction/fraction_basics.rs b/akari/src/fraction/fraction_basics.rs
--- a/akari/src/fraction/fraction_basics.rs
+++ b/akari/src/fraction/fraction_basics.rs
@@ -248,6 +248,17 @@ fn test_checked_operations() {
     let zero = Fraction::new(0, 1);
     assert_eq!(a.checked_div(zero), None);
     println!("checked_div by zero = {:?}", a.checked_div(zero)); // None
+
+    // Intermediates use wider integers, so reducible results do not overflow early.
+    assert_eq!(
+        Fraction::new(i64::MAX, 2).checked_mul(Fraction::new(2, i64::MAX)),
+        Some(Fraction::from_integer(1))
+    );
+
+    assert_eq!(
+        Fraction::from_integer(i64::MAX).checked_add(Fraction::from_integer(1)),
+        None
+    );
 }
 
 // ─────────────────────────────────────────────────────────────────
```

If `git apply` reports that the hunk line number has moved, that is harmless as long as the surrounding `checked_div` context matches. `git apply` normally locates it automatically.

### Verify Stage 3

```bash
cargo test -p akari fraction::fraction_basics::test_checked_operations
```

## Stage 4: Test invalid float denominator bounds

### Easy explanation

The float conversion accepts a maximum denominator. A maximum of zero or less cannot describe a valid positive denominator, so it should return `None` immediately.

### Technical details

Before the production change in Stage 1, the continued-fraction loop could stop before its first convergent and return a misleading integer. The guard `max_denom <= 0` makes the input contract explicit.

### Patch

```diff
diff --git a/akari/src/fraction/fraction_basics.rs b/akari/src/fraction/fraction_basics.rs
--- a/akari/src/fraction/fraction_basics.rs
+++ b/akari/src/fraction/fraction_basics.rs
@@ -339,6 +339,8 @@ fn test_from_f64_bounded() {
     assert_eq!(Fraction::from_f64_bounded(f64::NAN, 100), None);
     assert_eq!(Fraction::from_f64_bounded(f64::INFINITY, 100), None);
     assert_eq!(Fraction::from_f64_bounded(f64::NEG_INFINITY, 100), None);
+    assert_eq!(Fraction::from_f64_bounded(0.5, 0), None);
+    assert_eq!(Fraction::from_f64_bounded(0.5, -1), None);
 }
 
 /// `from_f64(f)` — same as `from_f64_bounded` with `max_denom = 1_000_000`.
```

### Verify Stage 4

```bash
cargo test -p akari fraction::fraction_basics::test_from_f64_bounded
```

## Final verification

After all four stages, run:

```bash
cargo test -p akari fraction::
cargo test -p akari
cargo test -p akari --doc
cargo check -p akari --no-default-features --features no_std
cargo clippy -p akari --lib --no-default-features -- \
  -A clippy::non_minimal_cfg -A clippy::module_inception -D warnings
git diff --check
```

Expected relevant results for this repository state:

- 16 fraction tests pass.
- 74 default unit tests pass.
- 104 documentation tests pass and 5 are ignored.
- The `no_std` check passes.
- Fraction-only Clippy passes with warnings denied; the two allowances are for existing crate structure outside this migration.

Do not use `cargo test --all-features` as the compatibility check for this repository: the feature set includes both `no_std` and `template`, and the crate deliberately rejects that combination. Use `--features full` for the standard-library feature bundle or the explicit commands above.

The current full-feature test suite also contains two unrelated template tests that require missing fixture files. Those failures are outside the fraction migration.

## Final API example

```rust
use akari::Fraction;
use core::num::NonZeroU64;

let half = Fraction::new(2, 4);
assert_eq!(half.numer(), 1);
assert_eq!(half.denom(), 2);

let negative = Fraction::new(1, -2);
assert_eq!(negative, Fraction::new(-1, 2));

let wide = Fraction::new_nonzero(1, NonZeroU64::new(u64::MAX).unwrap());
assert_eq!(wide.denom_nonzero().get(), u64::MAX);

assert_eq!(half.checked_add(half), Some(Fraction::from_integer(1)));
```

## Review checklist

Before considering the migration complete, confirm:

- No code can construct a stored zero denominator.
- Every constructor reduces the fraction.
- Zero always becomes `0/1`.
- Negative input denominators move their sign to the numerator.
- `i64::MIN` is handled through `unsigned_abs()`.
- Range checks happen after reduction.
- Division by a zero fraction returns `None` in `checked_div`.
- Regular operators never wrap silently in release mode.
- `denom()` returning `u64` is documented as an intentional API change.
