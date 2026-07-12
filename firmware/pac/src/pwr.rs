#[repr(C)]
#[doc = "Register block"]
pub struct RegisterBlock {
    cr0: Cr0,
    cr1: Cr1,
    cr2: Cr2,
    _reserved3: [u8; 0x04],
    sr0: Sr0,
    sr1: Sr1,
    gpreg0: Gpreg0,
    gpreg1: Gpreg1,
    cfgr: Cfgr,
    _reserved8: [u8; 0x04],
    anakey1: Anakey1,
    anakey2: Anakey2,
}
impl RegisterBlock {
    #[doc = "0x00 - Control register 0"]
    #[inline(always)]
    pub const fn cr0(&self) -> &Cr0 {
        &self.cr0
    }
    #[doc = "0x04 - Control register 1"]
    #[inline(always)]
    pub const fn cr1(&self) -> &Cr1 {
        &self.cr1
    }
    #[doc = "0x08 - Control register 2"]
    #[inline(always)]
    pub const fn cr2(&self) -> &Cr2 {
        &self.cr2
    }
    #[doc = "0x10 - Status register 0"]
    #[inline(always)]
    pub const fn sr0(&self) -> &Sr0 {
        &self.sr0
    }
    #[doc = "0x14 - Status register 1"]
    #[inline(always)]
    pub const fn sr1(&self) -> &Sr1 {
        &self.sr1
    }
    #[doc = "0x18 - General-purpose register 0"]
    #[inline(always)]
    pub const fn gpreg0(&self) -> &Gpreg0 {
        &self.gpreg0
    }
    #[doc = "0x1c - General-purpose register 1"]
    #[inline(always)]
    pub const fn gpreg1(&self) -> &Gpreg1 {
        &self.gpreg1
    }
    #[doc = "0x20 - Configuration register"]
    #[inline(always)]
    pub const fn cfgr(&self) -> &Cfgr {
        &self.cfgr
    }
    #[doc = "0x28 - ANCTL write-enable key register 1"]
    #[inline(always)]
    pub const fn anakey1(&self) -> &Anakey1 {
        &self.anakey1
    }
    #[doc = "0x2c - ANCTL write-enable key register 2"]
    #[inline(always)]
    pub const fn anakey2(&self) -> &Anakey2 {
        &self.anakey2
    }
}
#[doc = "CR0 (rw) register accessor: Control register 0\n\nYou can [`read`](crate::Reg::read) this register and get [`cr0::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`cr0::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@cr0`] module"]
#[doc(alias = "CR0")]
pub type Cr0 = crate::Reg<cr0::Cr0Spec>;
#[doc = "Control register 0"]
pub mod cr0;
#[doc = "CR1 (w) register accessor: Control register 1\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`cr1::W`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@cr1`] module"]
#[doc(alias = "CR1")]
pub type Cr1 = crate::Reg<cr1::Cr1Spec>;
#[doc = "Control register 1"]
pub mod cr1;
#[doc = "CR2 (rw) register accessor: Control register 2\n\nYou can [`read`](crate::Reg::read) this register and get [`cr2::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`cr2::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@cr2`] module"]
#[doc(alias = "CR2")]
pub type Cr2 = crate::Reg<cr2::Cr2Spec>;
#[doc = "Control register 2"]
pub mod cr2;
#[doc = "SR0 (r) register accessor: Status register 0\n\nYou can [`read`](crate::Reg::read) this register and get [`sr0::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@sr0`] module"]
#[doc(alias = "SR0")]
pub type Sr0 = crate::Reg<sr0::Sr0Spec>;
#[doc = "Status register 0"]
pub mod sr0;
#[doc = "SR1 (r) register accessor: Status register 1\n\nYou can [`read`](crate::Reg::read) this register and get [`sr1::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@sr1`] module"]
#[doc(alias = "SR1")]
pub type Sr1 = crate::Reg<sr1::Sr1Spec>;
#[doc = "Status register 1"]
pub mod sr1;
#[doc = "GPREG0 (rw) register accessor: General-purpose register 0\n\nYou can [`read`](crate::Reg::read) this register and get [`gpreg0::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`gpreg0::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@gpreg0`] module"]
#[doc(alias = "GPREG0")]
pub type Gpreg0 = crate::Reg<gpreg0::Gpreg0Spec>;
#[doc = "General-purpose register 0"]
pub mod gpreg0;
#[doc = "GPREG1 (rw) register accessor: General-purpose register 1\n\nYou can [`read`](crate::Reg::read) this register and get [`gpreg1::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`gpreg1::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@gpreg1`] module"]
#[doc(alias = "GPREG1")]
pub type Gpreg1 = crate::Reg<gpreg1::Gpreg1Spec>;
#[doc = "General-purpose register 1"]
pub mod gpreg1;
#[doc = "CFGR (rw) register accessor: Configuration register\n\nYou can [`read`](crate::Reg::read) this register and get [`cfgr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`cfgr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@cfgr`] module"]
#[doc(alias = "CFGR")]
pub type Cfgr = crate::Reg<cfgr::CfgrSpec>;
#[doc = "Configuration register"]
pub mod cfgr;
#[doc = "ANAKEY1 (w) register accessor: ANCTL write-enable key register 1\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`anakey1::W`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@anakey1`] module"]
#[doc(alias = "ANAKEY1")]
pub type Anakey1 = crate::Reg<anakey1::Anakey1Spec>;
#[doc = "ANCTL write-enable key register 1"]
pub mod anakey1;
#[doc = "ANAKEY2 (w) register accessor: ANCTL write-enable key register 2\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`anakey2::W`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@anakey2`] module"]
#[doc(alias = "ANAKEY2")]
pub type Anakey2 = crate::Reg<anakey2::Anakey2Spec>;
#[doc = "ANCTL write-enable key register 2"]
pub mod anakey2;
