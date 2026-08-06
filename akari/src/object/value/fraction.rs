use core::fmt;
use core::ops::{
    Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign, Sub, SubAssign,
};

/// Helper function to compute Greatest Common Divisor (GCD) using Euclidean algorithm.
fn gcd(mut a: i64, mut b: i64) -> i64 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    if a == 0 { 1 } else { a }
}

/// Represents an exact rational fraction `numerator / denominator`.
///
/// Under the hood, fractions are kept reduced to lowest terms, with the denominator
/// guaranteed to be strictly positive (`> 0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fraction {
    numer: i64,
    denom: i64,
}

impl Fraction {
    /// Default denominator cap for continued-fraction approximation.
    ///
    /// This keeps decimal-to-fraction conversion precise for common inputs
    /// while still bounding the search cost.
    pub const DEFAULT_MAX_DENOM: i64 = 1_000_000;

    /// Creates a new `Fraction` reduced to lowest terms.
    pub fn new(numer: i64, denom: i64) -> Self {
        Self::try_new(numer, denom).expect("Fraction::new requires a non-zero denominator")
    }

    /// Fallible constructor that returns `None` when the denominator is zero.
    pub fn try_new(numer: i64, denom: i64) -> Option<Self> {
        if denom == 0 {
            return None;
        }
        let g = gcd(numer, denom);
        let mut n = numer / g;
        let mut d = denom / g;
        if d < 0 {
            n = -n;
            d = -d;
        }
        Some(Self { numer: n, denom: d })
    }

    /// Creates a fraction from an integer `n / 1`.
    pub fn from_integer(n: i64) -> Self {
        Self { numer: n, denom: 1 }
    }

    /// Returns the numerator of the fraction.
    pub fn numer(&self) -> i64 {
        self.numer
    }

    /// Returns the denominator of the fraction (guaranteed > 0).
    pub fn denom(&self) -> i64 {
        self.denom
    }

    /// Converts the fraction to an `f64` representation.
    pub fn to_f64(&self) -> f64 {
        self.numer as f64 / self.denom as f64
    }

    /// Returns true if the fraction is an integer (denominator is 1).
    pub fn is_integer(&self) -> bool {
        self.denom == 1
    }

    /// Converts an `f64` to the best rational approximation with denominator ≤ `max_denom`,
    /// using the **continued fraction algorithm**.
    ///
    /// Returns `None` if `f` is NaN or infinite.
    ///
    /// # Algorithm
    /// The continued fraction expansion of `f` is computed iteratively.  Each convergent
    /// `h/k` is the best rational approximation for its denominator size.  The loop stops
    /// when the next convergent would exceed `max_denom`, leaving the last good convergent
    /// as the result.
    ///
    /// # Examples
    /// ```
    /// use akari::Fraction;
    /// assert_eq!(Fraction::from_f64_bounded(1.5, 100),   Some(Fraction::new(3, 2)));
    /// assert_eq!(Fraction::from_f64_bounded(0.1, 10),    Some(Fraction::new(1, 10)));
    /// assert_eq!(Fraction::from_f64_bounded(3.14159, 10), Some(Fraction::new(22, 7)));
    /// assert_eq!(Fraction::from_f64_bounded(f64::NAN, 100), None);
    /// ```
    pub fn from_f64_bounded(f: f64, max_denom: i64) -> Option<Self> {
        if !f.is_finite() {
            return None;
        }
        let sign: i64 = if f < 0.0 { -1 } else { 1 };
        let x = f.abs();

        // Continued-fraction recurrence:
        //   h_{-1}=0  h_0=1
        //   k_{-1}=1  k_0=0
        //   a_n = floor(rem)
        //   h_n = a_n * h_{n-1} + h_{n-2}
        //   k_n = a_n * k_{n-1} + k_{n-2}
        let mut h_prev: i64 = 0;
        let mut h_curr: i64 = 1;
        let mut k_prev: i64 = 1;
        let mut k_curr: i64 = 0;
        let mut rem = x;

        loop {
            let a = rem as i64; // floor
            let h_next = a.checked_mul(h_curr)?.checked_add(h_prev)?;
            let k_next = a.checked_mul(k_curr)?.checked_add(k_prev)?;

            if k_next > max_denom {
                break;
            }

            h_prev = h_curr;
            h_curr = h_next;
            k_prev = k_curr;
            k_curr = k_next;

            let frac_part = rem - a as f64;
            if frac_part < 1e-12 {
                break; // exact representation reached
            }
            rem = 1.0 / frac_part;
        }

        // k_curr == 0 only before the first iteration (x == 0 or max_denom == 0).
        if k_curr == 0 {
            Some(Self::from_integer(sign * h_curr))
        } else {
            Some(Self::new(sign * h_curr, k_curr))
        }
    }

    /// Converts an `f64` to the best rational approximation with denominator ≤ 1,000,000.
    ///
    /// Returns `None` if `f` is NaN or infinite.
    ///
    /// # Examples
    /// ```
    /// use akari::Fraction;
    /// assert_eq!(Fraction::from_f64(0.25), Some(Fraction::new(1, 4)));
    /// assert_eq!(Fraction::from_f64(0.1),  Some(Fraction::new(1, 10)));
    /// assert_eq!(Fraction::from_f64(1.0 / 3.0), Some(Fraction::new(1, 3)));
    /// ```
    pub fn from_f64(f: f64) -> Option<Self> {
        Self::from_f64_bounded(f, Self::DEFAULT_MAX_DENOM)
    }

    /// Converts an `f64` to the best rational approximation whose error is ≤ `tolerance`.
    ///
    /// Internally uses [`from_f64_bounded`](Self::from_f64_bounded) with `max_denom =
    /// 1_000_000` and then checks that the approximation is within `tolerance`.
    /// Returns `None` if no such fraction exists or `f` is not finite.
    ///
    /// # Examples
    /// ```
    /// use akari::Fraction;
    /// // 0.333 is exactly 333/1000 in floating-point; the CF best approximation
    /// // is 333/1000, which is within 0.01 of 0.333, so it is returned.
    /// assert_eq!(Fraction::from_f64_tol(0.333, 0.01), Some(Fraction::new(333, 1000)));
    /// // Tolerance is still met (333/1000 matches 0.333 exactly in f64)
    /// assert!(Fraction::from_f64_tol(0.333, 1e-6).is_some());
    /// // NaN and infinite values always return None
    /// assert_eq!(Fraction::from_f64_tol(f64::NAN, 1.0), None);
    /// assert_eq!(Fraction::from_f64_tol(f64::INFINITY, 1.0), None);
    /// ```
    pub fn from_f64_tol(f: f64, tolerance: f64) -> Option<Self> {
        let approx = Self::from_f64(f)?;
        if (approx.to_f64() - f).abs() <= tolerance {
            Some(approx)
        } else {
            None
        }
    }

    /// Infallible conversion from `f64`.  Uses the continued-fraction algorithm
    /// (max denominator 1,000,000) when the value is finite; falls back to
    /// integer truncation for NaN / infinity.
    ///
    /// # Examples
    /// ```
    /// use akari::Fraction;
    /// assert_eq!(Fraction::approx_f64(1.5),  Fraction::new(3, 2));
    /// assert_eq!(Fraction::approx_f64(0.1),  Fraction::new(1, 10));
    /// assert_eq!(Fraction::approx_f64(f64::NAN), Fraction::new(0, 1)); // fallback
    /// ```
    pub fn approx_f64(f: f64) -> Self {
        Self::from_f64(f).unwrap_or_else(|| Self::from_integer(f as i64))
    }

    /// Checked addition. Returns `None` on overflow or divide-by-zero.
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        let n = self
            .numer
            .checked_mul(rhs.denom)?
            .checked_add(rhs.numer.checked_mul(self.denom)?)?;
        let d = self.denom.checked_mul(rhs.denom)?;
        Some(Self::new(n, d))
    }

    /// Checked subtraction.
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        let n = self
            .numer
            .checked_mul(rhs.denom)?
            .checked_sub(rhs.numer.checked_mul(self.denom)?)?;
        let d = self.denom.checked_mul(rhs.denom)?;
        Some(Self::new(n, d))
    }

    /// Checked multiplication.
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        let n = self.numer.checked_mul(rhs.numer)?;
        let d = self.denom.checked_mul(rhs.denom)?;
        Some(Self::new(n, d))
    }

    /// Checked division. Returns `None` if rhs numerator is 0.
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.numer == 0 {
            return None;
        }
        let n = self.numer.checked_mul(rhs.denom)?;
        let d = self.denom.checked_mul(rhs.numer)?;
        Some(Self::new(n, d))
    }
}

impl Default for Fraction {
    fn default() -> Self {
        Self { numer: 0, denom: 1 }
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Fraction {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let lhs = self.numer as i128 * other.denom as i128;
        let rhs = other.numer as i128 * self.denom as i128;
        lhs.cmp(&rhs)
    }
}

impl fmt::Display for Fraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.denom == 1 {
            write!(f, "{}", self.numer)
        } else {
            write!(f, "{}/{}", self.numer, self.denom)
        }
    }
}

impl Add for Fraction {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(
            self.numer * rhs.denom + rhs.numer * self.denom,
            self.denom * rhs.denom,
        )
    }
}

impl Sub for Fraction {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(
            self.numer * rhs.denom - rhs.numer * self.denom,
            self.denom * rhs.denom,
        )
    }
}

impl Mul for Fraction {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.numer * rhs.numer, self.denom * rhs.denom)
    }
}

impl Div for Fraction {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Self::new(self.numer * rhs.denom, self.denom * rhs.numer)
    }
}

impl Rem for Fraction {
    type Output = Self;
    fn rem(self, rhs: Self) -> Self::Output {
        let div = self / rhs;
        let int_part = div.numer / div.denom;
        self - (rhs * Fraction::from_integer(int_part))
    }
}

impl Neg for Fraction {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self {
            numer: -self.numer,
            denom: self.denom,
        }
    }
}

impl AddAssign for Fraction {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Fraction {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl MulAssign for Fraction {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl DivAssign for Fraction {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

impl RemAssign for Fraction {
    fn rem_assign(&mut self, rhs: Self) {
        *self = *self % rhs;
    }
}

impl From<i64> for Fraction {
    fn from(n: i64) -> Self {
        Self::from_integer(n)
    }
}

impl From<i32> for Fraction {
    fn from(n: i32) -> Self {
        Self::from_integer(n as i64)
    }
}

impl From<(i64, i64)> for Fraction {
    fn from((numer, denom): (i64, i64)) -> Self {
        Self::new(numer, denom)
    }
}
