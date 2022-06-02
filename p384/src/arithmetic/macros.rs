//! Field arithmetic macros

// TODO(tarcieri): extract this into the `elliptic-curve` crate when stable

/// Provides both inherent and trait impls for a field element type which are
/// backed by a core set of arithmetic functions specified as macro arguments.
///
/// # Inherent impls
/// - `const ZERO: Self`
/// - `const ONE: Self` (multiplicative identity)
/// - `pub fn from_be_bytes`
/// - `pub fn from_be_slice`
/// - `pub fn from_le_bytes`
/// - `pub fn from_le_slice`
/// - `pub fn from_uint`
/// - `fn from_uint_unchecked`
/// - `pub fn to_be_bytes`
/// - `pub fn to_le_bytes`
/// - `pub fn to_canonical`
/// - `pub fn is_odd`
/// - `pub fn is_zero`
/// - `pub fn double`
/// - `pub fn invert`
///
/// NOTE: field implementations must provide their own inherent `pub fn sqrt`
/// method in order for the code generated by this macro to compile.
///
/// # Trait impls
/// - `AsRef<$arr>`
/// - `ConditionallySelectable`
/// - `ConstantTimeEq`
/// - `ConstantTimeGreater`
/// - `ConstantTimeLess`
/// - `Default`
/// - `DefaultIsZeroes`
/// - `Eq`
/// - `Field`
/// - `PartialEq`
///
/// ## Ops
/// - `Add`
/// - `AddAssign`
/// - `Sub`
/// - `SubAssign`
/// - `Mul`
/// - `MulAssign`
/// - `Neg`
macro_rules! impl_sec1_field_element {
    (
        $fe:tt,
        $uint:ty,
        $bytes:ty,
        $arr:ty,
        $from_mont:ident,
        $to_mont:ident,
        $add:ident,
        $sub:ident,
        $mul:ident,
        $neg:ident,
        $square:ident,
        $divstep_precomp:ident,
        $divstep:ident,
        $msat:ident,
        $mod:expr,
        $one:expr
    ) => {
        impl $fe {
            /// Zero element.
            pub const ZERO: Self = Self(<$uint>::ZERO);

            /// Multiplicative identity.
            pub const ONE: Self = Self(<$uint>::from_be_hex($one));

            /// Create a [`
            #[doc = stringify!($fe)]
            /// `] from a canonical big-endian representation.
            pub fn from_be_bytes(repr: $bytes) -> ::elliptic_curve::subtle::CtOption<Self> {
                Self::from_uint(<$uint>::from_be_byte_array(repr))
            }

            /// Decode [`
            #[doc = stringify!($fe)]
            /// `] from a big endian byte slice.
            pub fn from_be_slice(slice: &[u8]) -> ::elliptic_curve::Result<Self> {
                <$uint as ::elliptic_curve::bigint::Encoding>::Repr::try_from(slice)
                    .ok()
                    .and_then(|array| Self::from_be_bytes(array.into()).into())
                    .ok_or(::elliptic_curve::Error)
            }

            /// Create a [`
            #[doc = stringify!($fe)]
            /// `] from a canonical little-endian representation.
            pub fn from_le_bytes(repr: $bytes) -> ::elliptic_curve::subtle::CtOption<Self> {
                Self::from_uint(<$uint>::from_le_byte_array(repr))
            }

            /// Decode [`
            #[doc = stringify!($fe)]
            /// `] from a little endian byte slice.
            pub fn from_le_slice(slice: &[u8]) -> ::elliptic_curve::Result<Self> {
                <$uint as Encoding>::Repr::try_from(slice)
                    .ok()
                    .and_then(|array| Self::from_le_bytes(array.into()).into())
                    .ok_or(::elliptic_curve::Error)
            }

            /// Decode [`
            #[doc = stringify!($fe)]
            /// `]
            /// from [`
            #[doc = stringify!($uint)]
            /// `] converting it into Montgomery form:
            ///
            /// ```text
            /// w * R^2 * R^-1 mod p = wR mod p
            /// ```
            pub fn from_uint(uint: $uint) -> ::elliptic_curve::subtle::CtOption<Self> {
                let is_some = uint.ct_lt(&$mod);
                ::elliptic_curve::subtle::CtOption::new(Self::from_uint_unchecked(uint), is_some)
            }

            /// Decode [`
            #[doc = stringify!($fe)]
            /// `] from [`
            #[doc = stringify!($uint)]
            /// `] converting it into Montgomery form.
            ///
            /// Does not perform a check that the field element does not overflow the order.
            ///
            /// Used incorrectly this can lead to invalid results!
            fn from_uint_unchecked(w: $uint) -> Self {
                let mut mont = <$uint>::default();
                $to_mont(mont.as_mut(), w.as_ref());
                Self(mont)
            }

            /// Returns the big-endian encoding of this [`
            #[doc = stringify!($fe)]
            /// `].
            pub fn to_be_bytes(self) -> $bytes {
                self.to_canonical().to_be_byte_array()
            }

            /// Returns the little-endian encoding of this [`
            #[doc = stringify!($fe)]
            /// `].
            pub fn to_le_bytes(self) -> $bytes {
                self.to_canonical().to_le_byte_array()
            }

            /// Translate [`
            #[doc = stringify!($fe)]
            /// `] out of the Montgomery domain, returning a [`
            #[doc = stringify!($uint)]
            /// `] in canonical form.
            #[inline]
            pub fn to_canonical(self) -> $uint {
                let mut ret = <$uint>::default();
                $from_mont(ret.as_mut(), self.as_ref());
                ret
            }

            /// Determine if this [`
            #[doc = stringify!($fe)]
            /// `] is odd in the SEC1 sense: `self mod 2 == 1`.
            ///
            /// # Returns
            ///
            /// If odd, return `Choice(1)`.  Otherwise, return `Choice(0)`.
            pub fn is_odd(&self) -> Choice {
                self.to_canonical().is_odd()
            }

            /// Determine if this [`
            #[doc = stringify!($fe)]
            /// `] is zero.
            ///
            /// # Returns
            ///
            /// If zero, return `Choice(1)`.  Otherwise, return `Choice(0)`.
            pub fn is_zero(&self) -> Choice {
                self.ct_eq(&Self::ZERO)
            }

            /// Double element (add it to itself).
            #[must_use]
            pub fn double(&self) -> Self {
                self + self
            }

            /// Compute [`
            #[doc = stringify!($fe)]
            /// `] inversion: `1 / self`.
            pub fn invert(&self) -> ::elliptic_curve::subtle::CtOption<Self> {
                use ::elliptic_curve::{
                    bigint::{Limb, LimbUInt as Word},
                    subtle::ConditionallySelectable,
                };

                const LIMBS: usize = ::elliptic_curve::bigint::nlimbs!(<$uint>::BIT_SIZE);
                const ITERATIONS: usize = (49 * <$uint>::BIT_SIZE + 57) / 17;
                type XLimbs = [Word; LIMBS + 1];

                let mut d: Word = 1;
                let mut f = XLimbs::default();
                $msat(&mut f);

                let mut g = XLimbs::default();
                $from_mont((&mut g[..LIMBS]).try_into().unwrap(), self.as_ref());

                let mut r = <$arr>::from(Self::ONE.0);
                let mut v = <$arr>::default();
                let mut precomp = <$arr>::default();
                $divstep_precomp(&mut precomp);

                let mut out1 = Word::default();
                let mut out2 = XLimbs::default();
                let mut out3 = XLimbs::default();
                let mut out4 = <$arr>::default();
                let mut out5 = <$arr>::default();

                let mut i: usize = 0;

                while i < ITERATIONS - ITERATIONS % 2 {
                    $divstep(
                        &mut out1, &mut out2, &mut out3, &mut out4, &mut out5, d, &f, &g, &v, &r,
                    );
                    $divstep(
                        &mut d, &mut f, &mut g, &mut v, &mut r, out1, &out2, &out3, &out4, &out5,
                    );
                    i += 2;
                }

                if ITERATIONS % 2 != 0 {
                    $divstep(
                        &mut out1, &mut out2, &mut out3, &mut out4, &mut out5, d, &f, &g, &v, &r,
                    );
                    v = out4;
                    f = out2;
                }

                let mut v_opp = <$uint>::default();
                $neg(v_opp.as_mut(), &v);

                let v = <$uint>::from(v);

                let s = ::elliptic_curve::subtle::Choice::from(
                    ((f[f.len() - 1] >> Limb::BIT_SIZE - 1) & 1) as u8,
                );

                let v = <$uint>::conditional_select(&v, &v_opp, s);

                let mut ret = <$uint>::default();
                $mul(ret.as_mut(), v.as_ref(), &precomp);
                ::elliptic_curve::subtle::CtOption::new(Self(ret), !self.is_zero())
            }

            /// Compute modular square.
            #[must_use]
            pub fn square(&self) -> Self {
                let mut ret = <$uint>::default();
                $square(ret.as_mut(), self.as_ref());
                Self(ret)
            }
        }

        impl AsRef<$arr> for $fe {
            fn as_ref(&self) -> &$arr {
                self.0.as_ref()
            }
        }

        impl Default for $fe {
            fn default() -> Self {
                Self::ZERO
            }
        }

        impl Eq for $fe {}

        impl PartialEq for $fe {
            fn eq(&self, rhs: &Self) -> bool {
                self.0.ct_eq(&(rhs.0)).into()
            }
        }

        impl ::elliptic_curve::subtle::ConditionallySelectable for $fe {
            fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
                Self(<$uint>::conditional_select(&a.0, &b.0, choice))
            }
        }

        impl ::elliptic_curve::subtle::ConstantTimeEq for $fe {
            fn ct_eq(&self, other: &Self) -> ::elliptic_curve::subtle::Choice {
                self.0.ct_eq(&other.0)
            }
        }

        impl ::elliptic_curve::subtle::ConstantTimeGreater for $fe {
            fn ct_gt(&self, other: &Self) -> ::elliptic_curve::subtle::Choice {
                self.0.ct_gt(&other.0)
            }
        }

        impl ::elliptic_curve::subtle::ConstantTimeLess for $fe {
            fn ct_lt(&self, other: &Self) -> ::elliptic_curve::subtle::Choice {
                self.0.ct_lt(&other.0)
            }
        }

        impl ::elliptic_curve::zeroize::DefaultIsZeroes for $fe {}

        impl ::elliptic_curve::ff::Field for $fe {
            fn random(mut rng: impl ::elliptic_curve::rand_core::RngCore) -> Self {
                // NOTE: can't use ScalarCore::random due to CryptoRng bound
                let mut bytes = <$bytes>::default();

                loop {
                    rng.fill_bytes(&mut bytes);
                    if let Some(fe) = Self::from_be_bytes(bytes).into() {
                        return fe;
                    }
                }
            }

            fn zero() -> Self {
                Self::ZERO
            }

            fn one() -> Self {
                Self::ONE
            }

            fn is_zero(&self) -> Choice {
                Self::ZERO.ct_eq(self)
            }

            #[must_use]
            fn square(&self) -> Self {
                self.square()
            }

            #[must_use]
            fn double(&self) -> Self {
                self.double()
            }

            fn invert(&self) -> CtOption<Self> {
                self.invert()
            }

            fn sqrt(&self) -> CtOption<Self> {
                self.sqrt()
            }
        }

        impl_field_op!($fe, $uint, Add, add, $add);
        impl_field_op!($fe, $uint, Sub, sub, $sub);
        impl_field_op!($fe, $uint, Mul, mul, $mul);

        impl AddAssign<$fe> for $fe {
            #[inline]
            fn add_assign(&mut self, other: $fe) {
                *self = *self + other;
            }
        }

        impl AddAssign<&$fe> for $fe {
            #[inline]
            fn add_assign(&mut self, other: &$fe) {
                *self = *self + other;
            }
        }

        impl SubAssign<$fe> for $fe {
            #[inline]
            fn sub_assign(&mut self, other: $fe) {
                *self = *self - other;
            }
        }

        impl SubAssign<&$fe> for $fe {
            #[inline]
            fn sub_assign(&mut self, other: &$fe) {
                *self = *self - other;
            }
        }

        impl MulAssign<&$fe> for $fe {
            #[inline]
            fn mul_assign(&mut self, other: &$fe) {
                *self = *self * other;
            }
        }

        impl MulAssign for $fe {
            #[inline]
            fn mul_assign(&mut self, other: $fe) {
                *self = *self * other;
            }
        }

        impl Neg for $fe {
            type Output = $fe;

            #[inline]
            fn neg(self) -> $fe {
                let mut ret = <$uint>::default();
                $neg(ret.as_mut(), self.as_ref());
                Self(ret)
            }
        }
    };
}

/// Emit impls for a `core::ops` trait for all combinations of reference types,
/// which thunk to the given function.
macro_rules! impl_field_op {
    ($fe:tt, $uint:ty, $op:tt, $op_fn:ident, $func:ident) => {
        impl ::core::ops::$op for $fe {
            type Output = $fe;

            #[inline]
            fn $op_fn(self, rhs: $fe) -> $fe {
                let mut out = <$uint>::default();
                $func(out.as_mut(), self.as_ref(), rhs.as_ref());
                $fe(out)
            }
        }

        impl ::core::ops::$op<&$fe> for $fe {
            type Output = $fe;

            #[inline]
            fn $op_fn(self, rhs: &$fe) -> $fe {
                let mut out = <$uint>::default();
                $func(out.as_mut(), self.as_ref(), rhs.as_ref());
                $fe(out)
            }
        }

        impl ::core::ops::$op<&$fe> for &$fe {
            type Output = $fe;

            #[inline]
            fn $op_fn(self, rhs: &$fe) -> $fe {
                let mut out = <$uint>::default();
                $func(out.as_mut(), self.as_ref(), rhs.as_ref());
                $fe(out)
            }
        }
    };
}