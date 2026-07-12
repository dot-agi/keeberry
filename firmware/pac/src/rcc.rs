#[repr(C)]
#[doc = "Register block"]
pub struct RegisterBlock {
    pllpre: Pllpre,
    pllsrc: Pllsrc,
    mainclksrc: Mainclksrc,
    mainclkuen: Mainclkuen,
    _reserved4: [u8; 0x04],
    usbpre: Usbpre,
    ahbpre: Ahbpre,
    apb1pre: Apb1pre,
    apb2pre: Apb2pre,
    mclkpre: Mclkpre,
    i2spre: I2spre,
    mclksrc: Mclksrc,
    _reserved11: [u8; 0x04],
    usbfifoclksrc: Usbfifoclksrc,
    mcosel: Mcosel,
    ahbenr0: Ahbenr0,
    ahbenr1: Ahbenr1,
    ahbenr2: Ahbenr2,
    apb1enr: Apb1enr,
    apb2enr: Apb2enr,
    _reserved18: [u8; 0x0c],
    rngclkenr: Rngclkenr,
    pclkenr: Pclkenr,
    iwdgclkenr: Iwdgclkenr,
    _reserved21: [u8; 0x04],
    usbclkenr: Usbclkenr,
    i2sclkenr: I2sclkenr,
    spis1clkenr: Spis1clkenr,
    spis2clkenr: Spis2clkenr,
    usbfifoclkenr: Usbfifoclkenr,
    _reserved26: [u8; 0x08],
    ahbrstr1: Ahbrstr1,
    _reserved27: [u8; 0x04],
    apb1rstr: Apb1rstr,
    apb2rstr: Apb2rstr,
    _reserved29: [u8; 0x20],
    i2sclkrstr: I2sclkrstr,
    _reserved30: [u8; 0x0c],
    clrrststat: Clrrststat,
    _reserved31: [u8; 0x08],
    bdrstr: Bdrstr,
    lsi2rtcenr: Lsi2rtcenr,
    hse2rtcenr: Hse2rtcenr,
    _reserved34: [u8; 0x20],
    rststat: Rststat,
}
impl RegisterBlock {
    #[doc = "0x00 - PLL prescaler register"]
    #[inline(always)]
    pub const fn pllpre(&self) -> &Pllpre {
        &self.pllpre
    }
    #[doc = "0x04 - PLL source register"]
    #[inline(always)]
    pub const fn pllsrc(&self) -> &Pllsrc {
        &self.pllsrc
    }
    #[doc = "0x08 - Main clock source register"]
    #[inline(always)]
    pub const fn mainclksrc(&self) -> &Mainclksrc {
        &self.mainclksrc
    }
    #[doc = "0x0c - Main clock update enable register"]
    #[inline(always)]
    pub const fn mainclkuen(&self) -> &Mainclkuen {
        &self.mainclkuen
    }
    #[doc = "0x14 - USB prescaler register"]
    #[inline(always)]
    pub const fn usbpre(&self) -> &Usbpre {
        &self.usbpre
    }
    #[doc = "0x18 - AHB prescaler register"]
    #[inline(always)]
    pub const fn ahbpre(&self) -> &Ahbpre {
        &self.ahbpre
    }
    #[doc = "0x1c - APB1 prescaler register"]
    #[inline(always)]
    pub const fn apb1pre(&self) -> &Apb1pre {
        &self.apb1pre
    }
    #[doc = "0x20 - APB2 prescaler register"]
    #[inline(always)]
    pub const fn apb2pre(&self) -> &Apb2pre {
        &self.apb2pre
    }
    #[doc = "0x24 - MCLK prescaler register"]
    #[inline(always)]
    pub const fn mclkpre(&self) -> &Mclkpre {
        &self.mclkpre
    }
    #[doc = "0x28 - I2S prescaler register"]
    #[inline(always)]
    pub const fn i2spre(&self) -> &I2spre {
        &self.i2spre
    }
    #[doc = "0x2c - MCLK source register"]
    #[inline(always)]
    pub const fn mclksrc(&self) -> &Mclksrc {
        &self.mclksrc
    }
    #[doc = "0x34 - USB FIFO clock source register"]
    #[inline(always)]
    pub const fn usbfifoclksrc(&self) -> &Usbfifoclksrc {
        &self.usbfifoclksrc
    }
    #[doc = "0x38 - Microcontroller clock output select register"]
    #[inline(always)]
    pub const fn mcosel(&self) -> &Mcosel {
        &self.mcosel
    }
    #[doc = "0x3c - AHB peripheral clock enable register 0"]
    #[inline(always)]
    pub const fn ahbenr0(&self) -> &Ahbenr0 {
        &self.ahbenr0
    }
    #[doc = "0x40 - AHB peripheral clock enable register 1"]
    #[inline(always)]
    pub const fn ahbenr1(&self) -> &Ahbenr1 {
        &self.ahbenr1
    }
    #[doc = "0x44 - AHB peripheral clock enable register 2"]
    #[inline(always)]
    pub const fn ahbenr2(&self) -> &Ahbenr2 {
        &self.ahbenr2
    }
    #[doc = "0x48 - APB1 peripheral clock enable register"]
    #[inline(always)]
    pub const fn apb1enr(&self) -> &Apb1enr {
        &self.apb1enr
    }
    #[doc = "0x4c - APB2 peripheral clock enable register"]
    #[inline(always)]
    pub const fn apb2enr(&self) -> &Apb2enr {
        &self.apb2enr
    }
    #[doc = "0x5c - RNG clock enable register"]
    #[inline(always)]
    pub const fn rngclkenr(&self) -> &Rngclkenr {
        &self.rngclkenr
    }
    #[doc = "0x60 - Panel PCLK clock enable register"]
    #[inline(always)]
    pub const fn pclkenr(&self) -> &Pclkenr {
        &self.pclkenr
    }
    #[doc = "0x64 - Independent watchdog clock enable register"]
    #[inline(always)]
    pub const fn iwdgclkenr(&self) -> &Iwdgclkenr {
        &self.iwdgclkenr
    }
    #[doc = "0x6c - USB clock enable register"]
    #[inline(always)]
    pub const fn usbclkenr(&self) -> &Usbclkenr {
        &self.usbclkenr
    }
    #[doc = "0x70 - I2S SCLK enable register"]
    #[inline(always)]
    pub const fn i2sclkenr(&self) -> &I2sclkenr {
        &self.i2sclkenr
    }
    #[doc = "0x74 - SPIS1 clock enable register"]
    #[inline(always)]
    pub const fn spis1clkenr(&self) -> &Spis1clkenr {
        &self.spis1clkenr
    }
    #[doc = "0x78 - SPIS2 clock enable register"]
    #[inline(always)]
    pub const fn spis2clkenr(&self) -> &Spis2clkenr {
        &self.spis2clkenr
    }
    #[doc = "0x7c - USB FIFO clock enable register"]
    #[inline(always)]
    pub const fn usbfifoclkenr(&self) -> &Usbfifoclkenr {
        &self.usbfifoclkenr
    }
    #[doc = "0x88 - AHB peripheral reset register 1"]
    #[inline(always)]
    pub const fn ahbrstr1(&self) -> &Ahbrstr1 {
        &self.ahbrstr1
    }
    #[doc = "0x90 - APB1 peripheral reset register"]
    #[inline(always)]
    pub const fn apb1rstr(&self) -> &Apb1rstr {
        &self.apb1rstr
    }
    #[doc = "0x94 - APB2 peripheral reset register"]
    #[inline(always)]
    pub const fn apb2rstr(&self) -> &Apb2rstr {
        &self.apb2rstr
    }
    #[doc = "0xb8 - I2S SCLK reset register"]
    #[inline(always)]
    pub const fn i2sclkrstr(&self) -> &I2sclkrstr {
        &self.i2sclkrstr
    }
    #[doc = "0xc8 - Clear reset status register"]
    #[inline(always)]
    pub const fn clrrststat(&self) -> &Clrrststat {
        &self.clrrststat
    }
    #[doc = "0xd4 - Battery domain reset register"]
    #[inline(always)]
    pub const fn bdrstr(&self) -> &Bdrstr {
        &self.bdrstr
    }
    #[doc = "0xd8 - LSI-to-RTC clock enable register"]
    #[inline(always)]
    pub const fn lsi2rtcenr(&self) -> &Lsi2rtcenr {
        &self.lsi2rtcenr
    }
    #[doc = "0xdc - HSE-to-RTC clock enable register"]
    #[inline(always)]
    pub const fn hse2rtcenr(&self) -> &Hse2rtcenr {
        &self.hse2rtcenr
    }
    #[doc = "0x100 - Reset status register"]
    #[inline(always)]
    pub const fn rststat(&self) -> &Rststat {
        &self.rststat
    }
}
#[doc = "PLLPRE (rw) register accessor: PLL prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`pllpre::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pllpre::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@pllpre`] module"]
#[doc(alias = "PLLPRE")]
pub type Pllpre = crate::Reg<pllpre::PllpreSpec>;
#[doc = "PLL prescaler register"]
pub mod pllpre;
#[doc = "PLLSRC (rw) register accessor: PLL source register\n\nYou can [`read`](crate::Reg::read) this register and get [`pllsrc::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pllsrc::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@pllsrc`] module"]
#[doc(alias = "PLLSRC")]
pub type Pllsrc = crate::Reg<pllsrc::PllsrcSpec>;
#[doc = "PLL source register"]
pub mod pllsrc;
#[doc = "MAINCLKSRC (rw) register accessor: Main clock source register\n\nYou can [`read`](crate::Reg::read) this register and get [`mainclksrc::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mainclksrc::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@mainclksrc`] module"]
#[doc(alias = "MAINCLKSRC")]
pub type Mainclksrc = crate::Reg<mainclksrc::MainclksrcSpec>;
#[doc = "Main clock source register"]
pub mod mainclksrc;
#[doc = "MAINCLKUEN (rw) register accessor: Main clock update enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`mainclkuen::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mainclkuen::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@mainclkuen`] module"]
#[doc(alias = "MAINCLKUEN")]
pub type Mainclkuen = crate::Reg<mainclkuen::MainclkuenSpec>;
#[doc = "Main clock update enable register"]
pub mod mainclkuen;
#[doc = "USBPRE (rw) register accessor: USB prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbpre::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`usbpre::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@usbpre`] module"]
#[doc(alias = "USBPRE")]
pub type Usbpre = crate::Reg<usbpre::UsbpreSpec>;
#[doc = "USB prescaler register"]
pub mod usbpre;
#[doc = "AHBPRE (rw) register accessor: AHB prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`ahbpre::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ahbpre::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@ahbpre`] module"]
#[doc(alias = "AHBPRE")]
pub type Ahbpre = crate::Reg<ahbpre::AhbpreSpec>;
#[doc = "AHB prescaler register"]
pub mod ahbpre;
#[doc = "APB1PRE (rw) register accessor: APB1 prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`apb1pre::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`apb1pre::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@apb1pre`] module"]
#[doc(alias = "APB1PRE")]
pub type Apb1pre = crate::Reg<apb1pre::Apb1preSpec>;
#[doc = "APB1 prescaler register"]
pub mod apb1pre;
#[doc = "APB2PRE (rw) register accessor: APB2 prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`apb2pre::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`apb2pre::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@apb2pre`] module"]
#[doc(alias = "APB2PRE")]
pub type Apb2pre = crate::Reg<apb2pre::Apb2preSpec>;
#[doc = "APB2 prescaler register"]
pub mod apb2pre;
#[doc = "MCLKPRE (rw) register accessor: MCLK prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`mclkpre::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mclkpre::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@mclkpre`] module"]
#[doc(alias = "MCLKPRE")]
pub type Mclkpre = crate::Reg<mclkpre::MclkpreSpec>;
#[doc = "MCLK prescaler register"]
pub mod mclkpre;
#[doc = "I2SPRE (rw) register accessor: I2S prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`i2spre::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`i2spre::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@i2spre`] module"]
#[doc(alias = "I2SPRE")]
pub type I2spre = crate::Reg<i2spre::I2spreSpec>;
#[doc = "I2S prescaler register"]
pub mod i2spre;
#[doc = "MCLKSRC (rw) register accessor: MCLK source register\n\nYou can [`read`](crate::Reg::read) this register and get [`mclksrc::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mclksrc::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@mclksrc`] module"]
#[doc(alias = "MCLKSRC")]
pub type Mclksrc = crate::Reg<mclksrc::MclksrcSpec>;
#[doc = "MCLK source register"]
pub mod mclksrc;
#[doc = "USBFIFOCLKSRC (rw) register accessor: USB FIFO clock source register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbfifoclksrc::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`usbfifoclksrc::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@usbfifoclksrc`] module"]
#[doc(alias = "USBFIFOCLKSRC")]
pub type Usbfifoclksrc = crate::Reg<usbfifoclksrc::UsbfifoclksrcSpec>;
#[doc = "USB FIFO clock source register"]
pub mod usbfifoclksrc;
#[doc = "MCOSEL (rw) register accessor: Microcontroller clock output select register\n\nYou can [`read`](crate::Reg::read) this register and get [`mcosel::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mcosel::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@mcosel`] module"]
#[doc(alias = "MCOSEL")]
pub type Mcosel = crate::Reg<mcosel::McoselSpec>;
#[doc = "Microcontroller clock output select register"]
pub mod mcosel;
#[doc = "AHBENR0 (rw) register accessor: AHB peripheral clock enable register 0\n\nYou can [`read`](crate::Reg::read) this register and get [`ahbenr0::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ahbenr0::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@ahbenr0`] module"]
#[doc(alias = "AHBENR0")]
pub type Ahbenr0 = crate::Reg<ahbenr0::Ahbenr0Spec>;
#[doc = "AHB peripheral clock enable register 0"]
pub mod ahbenr0;
#[doc = "AHBENR1 (rw) register accessor: AHB peripheral clock enable register 1\n\nYou can [`read`](crate::Reg::read) this register and get [`ahbenr1::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ahbenr1::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@ahbenr1`] module"]
#[doc(alias = "AHBENR1")]
pub type Ahbenr1 = crate::Reg<ahbenr1::Ahbenr1Spec>;
#[doc = "AHB peripheral clock enable register 1"]
pub mod ahbenr1;
#[doc = "AHBENR2 (rw) register accessor: AHB peripheral clock enable register 2\n\nYou can [`read`](crate::Reg::read) this register and get [`ahbenr2::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ahbenr2::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@ahbenr2`] module"]
#[doc(alias = "AHBENR2")]
pub type Ahbenr2 = crate::Reg<ahbenr2::Ahbenr2Spec>;
#[doc = "AHB peripheral clock enable register 2"]
pub mod ahbenr2;
#[doc = "APB1ENR (rw) register accessor: APB1 peripheral clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`apb1enr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`apb1enr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@apb1enr`] module"]
#[doc(alias = "APB1ENR")]
pub type Apb1enr = crate::Reg<apb1enr::Apb1enrSpec>;
#[doc = "APB1 peripheral clock enable register"]
pub mod apb1enr;
#[doc = "APB2ENR (rw) register accessor: APB2 peripheral clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`apb2enr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`apb2enr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@apb2enr`] module"]
#[doc(alias = "APB2ENR")]
pub type Apb2enr = crate::Reg<apb2enr::Apb2enrSpec>;
#[doc = "APB2 peripheral clock enable register"]
pub mod apb2enr;
#[doc = "RNGCLKENR (rw) register accessor: RNG clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`rngclkenr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`rngclkenr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@rngclkenr`] module"]
#[doc(alias = "RNGCLKENR")]
pub type Rngclkenr = crate::Reg<rngclkenr::RngclkenrSpec>;
#[doc = "RNG clock enable register"]
pub mod rngclkenr;
#[doc = "PCLKENR (rw) register accessor: Panel PCLK clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`pclkenr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pclkenr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@pclkenr`] module"]
#[doc(alias = "PCLKENR")]
pub type Pclkenr = crate::Reg<pclkenr::PclkenrSpec>;
#[doc = "Panel PCLK clock enable register"]
pub mod pclkenr;
#[doc = "IWDGCLKENR (rw) register accessor: Independent watchdog clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`iwdgclkenr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`iwdgclkenr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@iwdgclkenr`] module"]
#[doc(alias = "IWDGCLKENR")]
pub type Iwdgclkenr = crate::Reg<iwdgclkenr::IwdgclkenrSpec>;
#[doc = "Independent watchdog clock enable register"]
pub mod iwdgclkenr;
#[doc = "USBCLKENR (rw) register accessor: USB clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbclkenr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`usbclkenr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@usbclkenr`] module"]
#[doc(alias = "USBCLKENR")]
pub type Usbclkenr = crate::Reg<usbclkenr::UsbclkenrSpec>;
#[doc = "USB clock enable register"]
pub mod usbclkenr;
#[doc = "I2SCLKENR (rw) register accessor: I2S SCLK enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`i2sclkenr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`i2sclkenr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@i2sclkenr`] module"]
#[doc(alias = "I2SCLKENR")]
pub type I2sclkenr = crate::Reg<i2sclkenr::I2sclkenrSpec>;
#[doc = "I2S SCLK enable register"]
pub mod i2sclkenr;
#[doc = "SPIS1CLKENR (rw) register accessor: SPIS1 clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`spis1clkenr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`spis1clkenr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@spis1clkenr`] module"]
#[doc(alias = "SPIS1CLKENR")]
pub type Spis1clkenr = crate::Reg<spis1clkenr::Spis1clkenrSpec>;
#[doc = "SPIS1 clock enable register"]
pub mod spis1clkenr;
#[doc = "SPIS2CLKENR (rw) register accessor: SPIS2 clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`spis2clkenr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`spis2clkenr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@spis2clkenr`] module"]
#[doc(alias = "SPIS2CLKENR")]
pub type Spis2clkenr = crate::Reg<spis2clkenr::Spis2clkenrSpec>;
#[doc = "SPIS2 clock enable register"]
pub mod spis2clkenr;
#[doc = "USBFIFOCLKENR (rw) register accessor: USB FIFO clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbfifoclkenr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`usbfifoclkenr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@usbfifoclkenr`] module"]
#[doc(alias = "USBFIFOCLKENR")]
pub type Usbfifoclkenr = crate::Reg<usbfifoclkenr::UsbfifoclkenrSpec>;
#[doc = "USB FIFO clock enable register"]
pub mod usbfifoclkenr;
#[doc = "AHBRSTR1 (rw) register accessor: AHB peripheral reset register 1\n\nYou can [`read`](crate::Reg::read) this register and get [`ahbrstr1::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ahbrstr1::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@ahbrstr1`] module"]
#[doc(alias = "AHBRSTR1")]
pub type Ahbrstr1 = crate::Reg<ahbrstr1::Ahbrstr1Spec>;
#[doc = "AHB peripheral reset register 1"]
pub mod ahbrstr1;
#[doc = "APB1RSTR (rw) register accessor: APB1 peripheral reset register\n\nYou can [`read`](crate::Reg::read) this register and get [`apb1rstr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`apb1rstr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@apb1rstr`] module"]
#[doc(alias = "APB1RSTR")]
pub type Apb1rstr = crate::Reg<apb1rstr::Apb1rstrSpec>;
#[doc = "APB1 peripheral reset register"]
pub mod apb1rstr;
#[doc = "APB2RSTR (rw) register accessor: APB2 peripheral reset register\n\nYou can [`read`](crate::Reg::read) this register and get [`apb2rstr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`apb2rstr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@apb2rstr`] module"]
#[doc(alias = "APB2RSTR")]
pub type Apb2rstr = crate::Reg<apb2rstr::Apb2rstrSpec>;
#[doc = "APB2 peripheral reset register"]
pub mod apb2rstr;
#[doc = "I2SCLKRSTR (rw) register accessor: I2S SCLK reset register\n\nYou can [`read`](crate::Reg::read) this register and get [`i2sclkrstr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`i2sclkrstr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@i2sclkrstr`] module"]
#[doc(alias = "I2SCLKRSTR")]
pub type I2sclkrstr = crate::Reg<i2sclkrstr::I2sclkrstrSpec>;
#[doc = "I2S SCLK reset register"]
pub mod i2sclkrstr;
#[doc = "CLRRSTSTAT (rw) register accessor: Clear reset status register\n\nYou can [`read`](crate::Reg::read) this register and get [`clrrststat::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`clrrststat::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@clrrststat`] module"]
#[doc(alias = "CLRRSTSTAT")]
pub type Clrrststat = crate::Reg<clrrststat::ClrrststatSpec>;
#[doc = "Clear reset status register"]
pub mod clrrststat;
#[doc = "BDRSTR (rw) register accessor: Battery domain reset register\n\nYou can [`read`](crate::Reg::read) this register and get [`bdrstr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`bdrstr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@bdrstr`] module"]
#[doc(alias = "BDRSTR")]
pub type Bdrstr = crate::Reg<bdrstr::BdrstrSpec>;
#[doc = "Battery domain reset register"]
pub mod bdrstr;
#[doc = "LSI2RTCENR (rw) register accessor: LSI-to-RTC clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`lsi2rtcenr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`lsi2rtcenr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@lsi2rtcenr`] module"]
#[doc(alias = "LSI2RTCENR")]
pub type Lsi2rtcenr = crate::Reg<lsi2rtcenr::Lsi2rtcenrSpec>;
#[doc = "LSI-to-RTC clock enable register"]
pub mod lsi2rtcenr;
#[doc = "HSE2RTCENR (rw) register accessor: HSE-to-RTC clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`hse2rtcenr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`hse2rtcenr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@hse2rtcenr`] module"]
#[doc(alias = "HSE2RTCENR")]
pub type Hse2rtcenr = crate::Reg<hse2rtcenr::Hse2rtcenrSpec>;
#[doc = "HSE-to-RTC clock enable register"]
pub mod hse2rtcenr;
#[doc = "RSTSTAT (rw) register accessor: Reset status register\n\nYou can [`read`](crate::Reg::read) this register and get [`rststat::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`rststat::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@rststat`] module"]
#[doc(alias = "RSTSTAT")]
pub type Rststat = crate::Reg<rststat::RststatSpec>;
#[doc = "Reset status register"]
pub mod rststat;
