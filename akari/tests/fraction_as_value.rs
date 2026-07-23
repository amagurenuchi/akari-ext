/// # Fraction as a Value variant
///
/// This test script shows how `Fraction` integrates into Akari's `Value`
/// dynamic type system alongside numbers, booleans, strings, lists, and dicts.
///
/// Key behaviours:
///  - `Value::Fraction(f)` is a first-class variant.
///  - `type_of()` returns `"frac"`.
///  - Arithmetic between two `Value::Fraction` stays exact (no floats).
///  - Cross-type arithmetic `Fraction op Numerical` converts the Numerical to the
///    closest Fraction via the continued-fraction algorithm (e.g. `1.5 → 3/2`),
///    so the result is always a `Value::Fraction`.
///  - Non-fallible accessors (`as_fraction`, `numerical`, `boolean`, `integer`)
///    all work on fraction values.
///  - `to_fraction()` converts any numeric Value to a Fraction.
///  - Fallible accessors (`try_as_fraction`, `try_to_fraction`) are also available.
///
/// Run with:
///   cargo test --test fraction_as_value --features full

use akari::{Value, Fraction};

// ─────────────────────────────────────────────────────────────────
// 1. Creating Value::Fraction
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_value_fraction_construction() {
    // From a Fraction directly
    let v: Value = Fraction::new(1, 2).into();
    assert!(matches!(v, Value::Fraction(_)));
    println!("type_of: {}", v.type_of()); // "frac"
    assert_eq!(v.type_of(), "frac");

    // Using Value::Fraction(..) constructor
    let v2 = Value::Fraction(Fraction::new(3, 4));
    println!("v2 = {}", v2); // "3/4"

    // From a (i64, i64) tuple via Into
    let v3: Value = (5_i64, 6_i64).into();
    assert_eq!(v3, Value::Fraction(Fraction::new(5, 6)));
    println!("(5,6).into() = {}", v3); // "5/6"

    // Using the convenience constructor
    let v4 = Value::fraction_of(7, 8);
    assert_eq!(v4, Value::Fraction(Fraction::new(7, 8)));

    // Default fraction Value is 0/1
    let v5 = Value::new_fraction();
    assert_eq!(v5, Value::Fraction(Fraction::new(0, 1)));
    println!("new_fraction() = {}", v5); // "0"
}

// ─────────────────────────────────────────────────────────────────
// 2. Inspecting a fraction Value
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_fraction_value_inspection() {
    let v = Value::Fraction(Fraction::new(3, 4));

    // is_fraction() – only true for Fraction variant
    assert!(v.is_fraction());
    assert!(!v.is_numerical());
    assert!(!v.is_boolean());
    assert!(!v.is_str());

    // as_fraction() – non-fallible, returns Fraction::default() on non-fraction
    assert_eq!(v.as_fraction(), Fraction::new(3, 4));
    let non_frac = Value::Str("hello".to_string());
    assert_eq!(non_frac.as_fraction(), Fraction::new(0, 1)); // default

    // fraction() is an alias for as_fraction()
    assert_eq!(v.fraction(), Fraction::new(3, 4));

    // as_fraction_or() with a custom default
    let default = Fraction::new(99, 100);
    let list_val = Value::new_list();
    assert_eq!(list_val.as_fraction_or(default), default);

    // try_as_fraction() – fallible, returns Ok or Err
    assert_eq!(v.try_as_fraction(), Ok(Fraction::new(3, 4)));
    let bad = Value::new_list();
    assert!(bad.try_as_fraction().is_err());

    // numerical() – converts to f64 (lossy, but convenient)
    assert!((v.numerical() - 0.75).abs() < 1e-12);

    // boolean() – false only when numerator == 0
    assert!(v.boolean());
    let zero_frac = Value::Fraction(Fraction::new(0, 5));
    assert!(!zero_frac.boolean());

    // integer() – integer part of the fraction (truncates)
    let big = Value::Fraction(Fraction::new(7, 3)); // 7/3 = 2.333...
    assert_eq!(big.integer(), 2);
}

// ─────────────────────────────────────────────────────────────────
// 3. Converting other types to Fraction
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_as_fraction_from_other_types() {
    // A Numerical value is converted via the CF algorithm, so integer-valued
    // floats give the exact integer fraction, and non-integers are handled precisely.
    let num_int = Value::Numerical(3.0);
    assert_eq!(num_int.as_fraction(), Fraction::new(3, 1));

    // 1.5 → 3/2  (CF algorithm, not truncation)
    let num_half = Value::Numerical(1.5);
    assert_eq!(num_half.as_fraction(), Fraction::new(3, 2));
    println!("Numerical(1.5).as_fraction() = {}", num_half.as_fraction()); // 3/2

    // 0.1 → 1/10
    let num_tenth = Value::Numerical(0.1);
    assert_eq!(num_tenth.as_fraction(), Fraction::new(1, 10));

    // Boolean: true → 1/1, false → 0/1
    let t = Value::Boolean(true);
    let f = Value::Boolean(false);
    assert_eq!(t.as_fraction(), Fraction::new(1, 1));
    assert_eq!(f.as_fraction(), Fraction::new(0, 1));

    // String "n/d" → parsed as fraction
    let s = Value::Str("2/5".to_string());
    assert_eq!(s.as_fraction(), Fraction::new(2, 5));
    println!("\"2/5\".as_fraction() = {}", s.as_fraction()); // 2/5

    // String "7" (integer) → 7/1
    let s2 = Value::Str("7".to_string());
    assert_eq!(s2.as_fraction(), Fraction::new(7, 1));

    // String "not a number" → default 0/1
    let s3 = Value::Str("not a number".to_string());
    assert_eq!(s3.as_fraction(), Fraction::new(0, 1));
}

// ─────────────────────────────────────────────────────────────────
// 4. Arithmetic between Value::Fraction values
// ─────────────────────────────────────────────────────────────────

/// Fraction + Fraction → Fraction (exact, no float involved)
#[test]
fn test_value_fraction_arithmetic_exact() {
    let a = Value::Fraction(Fraction::new(1, 2));
    let b = Value::Fraction(Fraction::new(1, 3));

    let sum = a.add(&b);
    assert_eq!(sum, Value::Fraction(Fraction::new(5, 6)));
    println!("{} + {} = {}", a, b, sum); // 1/2 + 1/3 = 5/6

    let diff = a.sub(&b);
    assert_eq!(diff, Value::Fraction(Fraction::new(1, 6)));
    println!("{} - {} = {}", a, b, diff); // 1/2 - 1/3 = 1/6

    let prod = a.mul(&b);
    assert_eq!(prod, Value::Fraction(Fraction::new(1, 6)));
    println!("{} * {} = {}", a, b, prod); // 1/2 * 1/3 = 1/6

    let quot = a.div(&b);
    // (1/2) / (1/3) = 3/2
    assert_eq!(quot, Value::Fraction(Fraction::new(3, 2)));
    println!("{} / {} = {}", a, b, quot); // 1/2 / 1/3 = 3/2
}

/// Fraction + Numerical → Fraction (Numerical is converted via the continued-fraction algorithm)
///
/// `Fraction::approx_f64(b)` finds the simplest fraction within `max_denom = 1_000_000`
/// that equals `b`. Integer-valued floats give an exact `n/1`; non-integer floats are
/// represented precisely where possible (e.g. `1.5 → 3/2`, `0.1 → 1/10`, `1.9 → 19/10`).
#[test]
fn test_value_fraction_mixed_with_float() {
    let frac = Value::Fraction(Fraction::new(1, 4)); // 1/4
    let num  = Value::Numerical(2.0);                // integer-valued float → 2/1

    let sum = frac.add(&num);
    // 1/4 + 2/1 = 9/4
    assert!(matches!(sum, Value::Fraction(_)), "Fraction + Numerical should stay Fraction");
    assert_eq!(sum, Value::Fraction(Fraction::new(9, 4)));
    println!("{} + {} = {}", frac, num, sum); // 1/4 + 2 = 9/4

    // Non-integer Numerical: 1.5 → 3/2 (exact via CF algorithm)
    let half = Value::Numerical(1.5);
    let result = frac.add(&half);
    // 1/4 + 3/2 = 1/4 + 6/4 = 7/4
    assert_eq!(result, Value::Fraction(Fraction::new(7, 4)));
    println!("{} + 1.5 = {}", frac, result); // 1/4 + 3/2 = 7/4

    // 1.9 → 19/10 (exact via CF algorithm)
    let non_int = Value::Numerical(1.9);
    let result2 = frac.add(&non_int);
    // 1/4 + 19/10 = 5/20 + 38/20 = 43/20
    assert_eq!(result2, Value::Fraction(Fraction::new(43, 20)));
    println!("{} + 1.9 = {}", frac, result2); // 1/4 + 19/10 = 43/20

    // If you need an f64 result, call .numerical() on the Fraction
    assert!((sum.numerical() - 2.25).abs() < 1e-12);
}



/// Fraction + Boolean → Fraction (booleans are treated as 0 or 1)
#[test]
fn test_value_fraction_mixed_with_boolean() {
    let frac  = Value::Fraction(Fraction::new(1, 3));
    let t_val = Value::Boolean(true);   // treated as 1/1
    let f_val = Value::Boolean(false);  // treated as 0/1

    let plus_true = frac.add(&t_val);
    // 1/3 + 1 = 4/3
    assert_eq!(plus_true, Value::Fraction(Fraction::new(4, 3)));
    println!("{} + true = {}", frac, plus_true); // 1/3 + true = 4/3

    let plus_false = frac.add(&f_val);
    // 1/3 + 0 = 1/3
    assert_eq!(plus_false, Value::Fraction(Fraction::new(1, 3)));
}

// ─────────────────────────────────────────────────────────────────
// 5. Division by zero is handled gracefully
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_value_fraction_division_by_zero() {
    let a    = Value::Fraction(Fraction::new(1, 2));
    let zero = Value::Fraction(Fraction::new(0, 1));

    // Non-fallible div returns Value::None on divide-by-zero
    let result = a.div(&zero);
    assert!(matches!(result, Value::None), "div by zero fraction → None");
    println!("{} / 0 = {:?}", a, result); // None

    // Fallible try_div returns Err
    let err = a.try_div(&zero);
    assert!(err.is_err(), "try_div by zero fraction → Err");
    println!("try_div by zero: {:?}", err);
}

// ─────────────────────────────────────────────────────────────────
// 6. Operator overloads (+=, -=, etc.)
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_value_fraction_operator_overloads() {
    let mut v = Value::Fraction(Fraction::new(1, 2));

    v += Value::Fraction(Fraction::new(1, 6));
    // 1/2 + 1/6 = 3/6 + 1/6 = 4/6 = 2/3
    assert_eq!(v, Value::Fraction(Fraction::new(2, 3)));
    println!("after += 1/6: {}", v); // 2/3

    v -= Value::Fraction(Fraction::new(1, 3));
    // 2/3 - 1/3 = 1/3
    assert_eq!(v, Value::Fraction(Fraction::new(1, 3)));
    println!("after -= 1/3: {}", v); // 1/3

    v *= Value::Fraction(Fraction::new(3, 1));
    // 1/3 * 3 = 1
    assert_eq!(v, Value::Fraction(Fraction::new(1, 1)));
    println!("after *= 3: {}", v); // 1
}

// ─────────────────────────────────────────────────────────────────
// 7. Equality and comparison across types
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_value_fraction_equality() {
    let frac = Value::Fraction(Fraction::new(2, 4)); // = 1/2
    let also_half = Value::Fraction(Fraction::new(1, 2));

    // Both reduce to 1/2, so they're equal
    assert_eq!(frac, also_half);

    // A Fraction equal in value to a Numerical is considered equal
    let as_float = Value::Numerical(0.5);
    assert_eq!(frac, as_float, "Fraction(1/2) == Numerical(0.5)");

    // Different values are not equal
    let third = Value::Fraction(Fraction::new(1, 3));
    assert_ne!(frac, third);
}

// ─────────────────────────────────────────────────────────────────
// 8. to_fraction / try_to_fraction
// ─────────────────────────────────────────────────────────────────

/// `to_fraction()` converts a `Value::Numerical` or `Value::Boolean` to
/// `Value::Fraction` using the CF algorithm.  Other types pass through unchanged.
#[test]
fn test_to_fraction() {
    // Integer-valued float → exact fraction
    let v = Value::Numerical(4.0);
    assert_eq!(v.to_fraction(), Value::Fraction(Fraction::new(4, 1)));
    println!("Numerical(4.0).to_fraction() = {}", v.to_fraction());

    // Non-integer float → CF approximation
    let v = Value::Numerical(1.5);
    assert_eq!(v.to_fraction(), Value::Fraction(Fraction::new(3, 2)));
    println!("Numerical(1.5).to_fraction() = {}", v.to_fraction());

    let v = Value::Numerical(0.1);
    assert_eq!(v.to_fraction(), Value::Fraction(Fraction::new(1, 10)));

    let v = Value::Numerical(1.0 / 3.0);
    assert_eq!(v.to_fraction(), Value::Fraction(Fraction::new(1, 3)));
    println!("Numerical(1/3).to_fraction() = {}", v.to_fraction()); // 1/3

    // Boolean
    assert_eq!(Value::Boolean(true).to_fraction(),  Value::Fraction(Fraction::new(1, 1)));
    assert_eq!(Value::Boolean(false).to_fraction(), Value::Fraction(Fraction::new(0, 1)));

    // Fraction pass-through — already a Fraction, returned as-is
    let f = Value::Fraction(Fraction::new(5, 7));
    assert_eq!(f.to_fraction(), f);

    // String pass-through — to_fraction doesn't parse strings, returns them unchanged
    let s = Value::Str("hello".to_string());
    assert_eq!(s.to_fraction(), s);

    // List and Dict pass through unchanged
    let list = Value::new_list();
    assert_eq!(list.to_fraction(), list);
}

/// `try_to_fraction()` is fallible — succeeds for Fraction, Numerical, Boolean,
/// and parseable strings; fails for un-parseable strings, lists, dicts, and None.
#[test]
fn test_try_to_fraction() {
    // Fraction — Ok, unchanged
    let f = Value::Fraction(Fraction::new(3, 4));
    assert_eq!(f.try_to_fraction(), Ok(Value::Fraction(Fraction::new(3, 4))));

    // Numerical → Ok(Fraction)
    let v = Value::Numerical(2.5);
    assert_eq!(v.try_to_fraction(), Ok(Value::Fraction(Fraction::new(5, 2))));
    println!("Numerical(2.5).try_to_fraction() = {:?}", v.try_to_fraction());

    // Boolean → Ok(Fraction)
    assert_eq!(Value::Boolean(true).try_to_fraction(),  Ok(Value::Fraction(Fraction::new(1, 1))));
    assert_eq!(Value::Boolean(false).try_to_fraction(), Ok(Value::Fraction(Fraction::new(0, 1))));

    // Parseable string "n/d" → Ok(Fraction)
    let s = Value::Str("3/5".to_string());
    assert_eq!(s.try_to_fraction(), Ok(Value::Fraction(Fraction::new(3, 5))));

    // Parseable string integer → Ok(Fraction(n/1))
    let s2 = Value::Str("7".to_string());
    assert_eq!(s2.try_to_fraction(), Ok(Value::Fraction(Fraction::new(7, 1))));

    // Un-parseable string → Err
    let bad_str = Value::Str("not a number".to_string());
    assert!(bad_str.try_to_fraction().is_err());
    println!("try_to_fraction(\"not a number\") = {:?}", bad_str.try_to_fraction());

    // List → Err
    assert!(Value::new_list().try_to_fraction().is_err());

    // Dict → Err
    assert!(Value::new_dict().try_to_fraction().is_err());

    // None → Err
    assert!(Value::None.try_to_fraction().is_err());
}
