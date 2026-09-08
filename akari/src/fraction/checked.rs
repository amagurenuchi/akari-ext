use crate::fraction::definition::Fraction;

impl Fraction {
    /// Checked addition. Returns `None` on overflow or divide-by-zero.
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.checked_add_with_rhs_sign(rhs, false)
    }

    /// Checked subtraction.
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.checked_add_with_rhs_sign(rhs, true)
    }

    /// Checked multiplication.
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        let n = (self.numer.unsigned_abs() as u128) * (rhs.numer.unsigned_abs() as u128);
        let d = (self.denom.get() as u128) * (rhs.denom.get() as u128);
        Self::from_magnitudes(n, d, (self.numer < 0) ^ (rhs.numer < 0))
    }

    /// Checked division. Returns `None` if rhs numerator is 0.
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.numer == 0 {
            return None;
        }
        let n = (self.numer.unsigned_abs() as u128) * (rhs.denom.get() as u128);
        let d = (self.denom.get() as u128) * (rhs.numer.unsigned_abs() as u128);
        Self::from_magnitudes(n, d, (self.numer < 0) ^ (rhs.numer < 0))
    }

    fn checked_add_with_rhs_sign(self, rhs: Self, negate_rhs: bool) -> Option<Self> {
        let lhs = (self.numer.unsigned_abs() as u128) * (rhs.denom.get() as u128);
        let rhs_magnitude = (rhs.numer.unsigned_abs() as u128) * (self.denom.get() as u128);
        let lhs_negative = self.numer < 0;
        let rhs_negative = (rhs.numer < 0) ^ negate_rhs;

        let (numer, negative) = if lhs_negative == rhs_negative {
            (lhs.checked_add(rhs_magnitude)?, lhs_negative)
        } else if lhs >= rhs_magnitude {
            (lhs - rhs_magnitude, lhs_negative)
        } else {
            (rhs_magnitude - lhs, rhs_negative)
        };

        let denom = (self.denom.get() as u128) * (rhs.denom.get() as u128);
        Self::from_magnitudes(numer, denom, negative)
    }
}
