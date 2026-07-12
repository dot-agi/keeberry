#[repr(C)]
#[doc = "Register block"]
pub struct RegisterBlock {
    moder: Moder,
    otyper: Otyper,
    ospeedr: Ospeedr,
    pupdr: Pupdr,
    idr: Idr,
    odr: Odr,
    bsrr: Bsrr,
    lckr: Lckr,
    afrl: Afrl,
    afrh: Afrh,
    smit: Smit,
    current: Current,
    cfgmsk: Cfgmsk,
}
impl RegisterBlock {
    #[doc = "0x00 - Port mode register"]
    #[inline(always)]
    pub const fn moder(&self) -> &Moder {
        &self.moder
    }
    #[doc = "0x04 - Port output type register"]
    #[inline(always)]
    pub const fn otyper(&self) -> &Otyper {
        &self.otyper
    }
    #[doc = "0x08 - Port output speed register"]
    #[inline(always)]
    pub const fn ospeedr(&self) -> &Ospeedr {
        &self.ospeedr
    }
    #[doc = "0x0c - Port pull-up/pull-down register"]
    #[inline(always)]
    pub const fn pupdr(&self) -> &Pupdr {
        &self.pupdr
    }
    #[doc = "0x10 - Port input data register"]
    #[inline(always)]
    pub const fn idr(&self) -> &Idr {
        &self.idr
    }
    #[doc = "0x14 - Port output data register"]
    #[inline(always)]
    pub const fn odr(&self) -> &Odr {
        &self.odr
    }
    #[doc = "0x18 - Port bit set/reset register"]
    #[inline(always)]
    pub const fn bsrr(&self) -> &Bsrr {
        &self.bsrr
    }
    #[doc = "0x1c - Port configuration lock register"]
    #[inline(always)]
    pub const fn lckr(&self) -> &Lckr {
        &self.lckr
    }
    #[doc = "0x20 - Alternate function low register"]
    #[inline(always)]
    pub const fn afrl(&self) -> &Afrl {
        &self.afrl
    }
    #[doc = "0x24 - Alternate function high register"]
    #[inline(always)]
    pub const fn afrh(&self) -> &Afrh {
        &self.afrh
    }
    #[doc = "0x28 - Port Schmitt-trigger input configuration register (vendor extension)"]
    #[inline(always)]
    pub const fn smit(&self) -> &Smit {
        &self.smit
    }
    #[doc = "0x2c - Port drive-current configuration register (vendor extension)"]
    #[inline(always)]
    pub const fn current(&self) -> &Current {
        &self.current
    }
    #[doc = "0x30 - Port per-pin configuration write-mask register (vendor extension)"]
    #[inline(always)]
    pub const fn cfgmsk(&self) -> &Cfgmsk {
        &self.cfgmsk
    }
}
#[doc = "MODER (rw) register accessor: Port mode register\n\nYou can [`read`](crate::Reg::read) this register and get [`moder::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`moder::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@moder`] module"]
#[doc(alias = "MODER")]
pub type Moder = crate::Reg<moder::ModerSpec>;
#[doc = "Port mode register"]
pub mod moder;
#[doc = "OTYPER (rw) register accessor: Port output type register\n\nYou can [`read`](crate::Reg::read) this register and get [`otyper::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`otyper::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@otyper`] module"]
#[doc(alias = "OTYPER")]
pub type Otyper = crate::Reg<otyper::OtyperSpec>;
#[doc = "Port output type register"]
pub mod otyper;
#[doc = "OSPEEDR (rw) register accessor: Port output speed register\n\nYou can [`read`](crate::Reg::read) this register and get [`ospeedr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ospeedr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@ospeedr`] module"]
#[doc(alias = "OSPEEDR")]
pub type Ospeedr = crate::Reg<ospeedr::OspeedrSpec>;
#[doc = "Port output speed register"]
pub mod ospeedr;
#[doc = "PUPDR (rw) register accessor: Port pull-up/pull-down register\n\nYou can [`read`](crate::Reg::read) this register and get [`pupdr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pupdr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@pupdr`] module"]
#[doc(alias = "PUPDR")]
pub type Pupdr = crate::Reg<pupdr::PupdrSpec>;
#[doc = "Port pull-up/pull-down register"]
pub mod pupdr;
#[doc = "IDR (r) register accessor: Port input data register\n\nYou can [`read`](crate::Reg::read) this register and get [`idr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@idr`] module"]
#[doc(alias = "IDR")]
pub type Idr = crate::Reg<idr::IdrSpec>;
#[doc = "Port input data register"]
pub mod idr;
#[doc = "ODR (rw) register accessor: Port output data register\n\nYou can [`read`](crate::Reg::read) this register and get [`odr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`odr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@odr`] module"]
#[doc(alias = "ODR")]
pub type Odr = crate::Reg<odr::OdrSpec>;
#[doc = "Port output data register"]
pub mod odr;
#[doc = "BSRR (w) register accessor: Port bit set/reset register\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`bsrr::W`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@bsrr`] module"]
#[doc(alias = "BSRR")]
pub type Bsrr = crate::Reg<bsrr::BsrrSpec>;
#[doc = "Port bit set/reset register"]
pub mod bsrr;
#[doc = "LCKR (rw) register accessor: Port configuration lock register\n\nYou can [`read`](crate::Reg::read) this register and get [`lckr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`lckr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@lckr`] module"]
#[doc(alias = "LCKR")]
pub type Lckr = crate::Reg<lckr::LckrSpec>;
#[doc = "Port configuration lock register"]
pub mod lckr;
#[doc = "AFRL (rw) register accessor: Alternate function low register\n\nYou can [`read`](crate::Reg::read) this register and get [`afrl::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`afrl::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@afrl`] module"]
#[doc(alias = "AFRL")]
pub type Afrl = crate::Reg<afrl::AfrlSpec>;
#[doc = "Alternate function low register"]
pub mod afrl;
#[doc = "AFRH (rw) register accessor: Alternate function high register\n\nYou can [`read`](crate::Reg::read) this register and get [`afrh::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`afrh::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@afrh`] module"]
#[doc(alias = "AFRH")]
pub type Afrh = crate::Reg<afrh::AfrhSpec>;
#[doc = "Alternate function high register"]
pub mod afrh;
#[doc = "SMIT (rw) register accessor: Port Schmitt-trigger input configuration register (vendor extension)\n\nYou can [`read`](crate::Reg::read) this register and get [`smit::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`smit::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@smit`] module"]
#[doc(alias = "SMIT")]
pub type Smit = crate::Reg<smit::SmitSpec>;
#[doc = "Port Schmitt-trigger input configuration register (vendor extension)"]
pub mod smit;
#[doc = "CURRENT (rw) register accessor: Port drive-current configuration register (vendor extension)\n\nYou can [`read`](crate::Reg::read) this register and get [`current::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`current::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@current`] module"]
#[doc(alias = "CURRENT")]
pub type Current = crate::Reg<current::CurrentSpec>;
#[doc = "Port drive-current configuration register (vendor extension)"]
pub mod current;
#[doc = "CFGMSK (rw) register accessor: Port per-pin configuration write-mask register (vendor extension)\n\nYou can [`read`](crate::Reg::read) this register and get [`cfgmsk::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`cfgmsk::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@cfgmsk`] module"]
#[doc(alias = "CFGMSK")]
pub type Cfgmsk = crate::Reg<cfgmsk::CfgmskSpec>;
#[doc = "Port per-pin configuration write-mask register (vendor extension)"]
pub mod cfgmsk;
