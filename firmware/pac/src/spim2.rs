#[repr(C)]
#[doc = "Register block"]
pub struct RegisterBlock {
    cr0: Cr0,
    cr1: Cr1,
    spienr: Spienr,
    mwcr: Mwcr,
    ser: Ser,
    baudr: Baudr,
    txftlr: Txftlr,
    rxftlr: Rxftlr,
    txflr: Txflr,
    rxflr: Rxflr,
    sr: Sr,
    ier: Ier,
    isr: Isr,
    risr: Risr,
    txoicr: Txoicr,
    rxoicr: Rxoicr,
    rxuicr: Rxuicr,
    msticr: Msticr,
    icr: Icr,
    dmacr: Dmacr,
    dmatdlr: Dmatdlr,
    dmardlr: Dmardlr,
    _reserved22: [u8; 0x08],
    dr: Dr,
    _reserved23: [u8; 0x8c],
    rx_sample_dly: RxSampleDly,
    espicr: Espicr,
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
    #[doc = "0x08 - SPI enable register"]
    #[inline(always)]
    pub const fn spienr(&self) -> &Spienr {
        &self.spienr
    }
    #[doc = "0x0c - Microwire control register"]
    #[inline(always)]
    pub const fn mwcr(&self) -> &Mwcr {
        &self.mwcr
    }
    #[doc = "0x10 - Slave enable register"]
    #[inline(always)]
    pub const fn ser(&self) -> &Ser {
        &self.ser
    }
    #[doc = "0x14 - Baud rate select"]
    #[inline(always)]
    pub const fn baudr(&self) -> &Baudr {
        &self.baudr
    }
    #[doc = "0x18 - Transmit FIFO threshold level"]
    #[inline(always)]
    pub const fn txftlr(&self) -> &Txftlr {
        &self.txftlr
    }
    #[doc = "0x1c - Receive FIFO threshold level"]
    #[inline(always)]
    pub const fn rxftlr(&self) -> &Rxftlr {
        &self.rxftlr
    }
    #[doc = "0x20 - Transmit FIFO level register"]
    #[inline(always)]
    pub const fn txflr(&self) -> &Txflr {
        &self.txflr
    }
    #[doc = "0x24 - Receive FIFO level register"]
    #[inline(always)]
    pub const fn rxflr(&self) -> &Rxflr {
        &self.rxflr
    }
    #[doc = "0x28 - Status register"]
    #[inline(always)]
    pub const fn sr(&self) -> &Sr {
        &self.sr
    }
    #[doc = "0x2c - Interrupt enable register"]
    #[inline(always)]
    pub const fn ier(&self) -> &Ier {
        &self.ier
    }
    #[doc = "0x30 - Interrupt status register"]
    #[inline(always)]
    pub const fn isr(&self) -> &Isr {
        &self.isr
    }
    #[doc = "0x34 - Raw interrupt status register"]
    #[inline(always)]
    pub const fn risr(&self) -> &Risr {
        &self.risr
    }
    #[doc = "0x38 - Transmit FIFO overflow interrupt clear register"]
    #[inline(always)]
    pub const fn txoicr(&self) -> &Txoicr {
        &self.txoicr
    }
    #[doc = "0x3c - Receive FIFO overflow interrupt clear register"]
    #[inline(always)]
    pub const fn rxoicr(&self) -> &Rxoicr {
        &self.rxoicr
    }
    #[doc = "0x40 - Receive FIFO underflow interrupt clear register"]
    #[inline(always)]
    pub const fn rxuicr(&self) -> &Rxuicr {
        &self.rxuicr
    }
    #[doc = "0x44 - Multi-master interrupt clear register"]
    #[inline(always)]
    pub const fn msticr(&self) -> &Msticr {
        &self.msticr
    }
    #[doc = "0x48 - Interrupt clear register"]
    #[inline(always)]
    pub const fn icr(&self) -> &Icr {
        &self.icr
    }
    #[doc = "0x4c - DMA control register"]
    #[inline(always)]
    pub const fn dmacr(&self) -> &Dmacr {
        &self.dmacr
    }
    #[doc = "0x50 - DMA transmit data level"]
    #[inline(always)]
    pub const fn dmatdlr(&self) -> &Dmatdlr {
        &self.dmatdlr
    }
    #[doc = "0x54 - DMA receive data level"]
    #[inline(always)]
    pub const fn dmardlr(&self) -> &Dmardlr {
        &self.dmardlr
    }
    #[doc = "0x60 - Data register"]
    #[inline(always)]
    pub const fn dr(&self) -> &Dr {
        &self.dr
    }
    #[doc = "0xf0 - RX sample delay register"]
    #[inline(always)]
    pub const fn rx_sample_dly(&self) -> &RxSampleDly {
        &self.rx_sample_dly
    }
    #[doc = "0xf4 - Enhanced SPI control register"]
    #[inline(always)]
    pub const fn espicr(&self) -> &Espicr {
        &self.espicr
    }
}
#[doc = "CR0 (rw) register accessor: Control register 0\n\nYou can [`read`](crate::Reg::read) this register and get [`cr0::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`cr0::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@cr0`] module"]
#[doc(alias = "CR0")]
pub type Cr0 = crate::Reg<cr0::Cr0Spec>;
#[doc = "Control register 0"]
pub mod cr0;
#[doc = "CR1 (rw) register accessor: Control register 1\n\nYou can [`read`](crate::Reg::read) this register and get [`cr1::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`cr1::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@cr1`] module"]
#[doc(alias = "CR1")]
pub type Cr1 = crate::Reg<cr1::Cr1Spec>;
#[doc = "Control register 1"]
pub mod cr1;
#[doc = "SPIENR (rw) register accessor: SPI enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`spienr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`spienr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@spienr`] module"]
#[doc(alias = "SPIENR")]
pub type Spienr = crate::Reg<spienr::SpienrSpec>;
#[doc = "SPI enable register"]
pub mod spienr;
#[doc = "MWCR (rw) register accessor: Microwire control register\n\nYou can [`read`](crate::Reg::read) this register and get [`mwcr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mwcr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@mwcr`] module"]
#[doc(alias = "MWCR")]
pub type Mwcr = crate::Reg<mwcr::MwcrSpec>;
#[doc = "Microwire control register"]
pub mod mwcr;
#[doc = "SER (rw) register accessor: Slave enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`ser::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ser::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@ser`] module"]
#[doc(alias = "SER")]
pub type Ser = crate::Reg<ser::SerSpec>;
#[doc = "Slave enable register"]
pub mod ser;
#[doc = "BAUDR (rw) register accessor: Baud rate select\n\nYou can [`read`](crate::Reg::read) this register and get [`baudr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`baudr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@baudr`] module"]
#[doc(alias = "BAUDR")]
pub type Baudr = crate::Reg<baudr::BaudrSpec>;
#[doc = "Baud rate select"]
pub mod baudr;
#[doc = "TXFTLR (rw) register accessor: Transmit FIFO threshold level\n\nYou can [`read`](crate::Reg::read) this register and get [`txftlr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`txftlr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@txftlr`] module"]
#[doc(alias = "TXFTLR")]
pub type Txftlr = crate::Reg<txftlr::TxftlrSpec>;
#[doc = "Transmit FIFO threshold level"]
pub mod txftlr;
#[doc = "RXFTLR (rw) register accessor: Receive FIFO threshold level\n\nYou can [`read`](crate::Reg::read) this register and get [`rxftlr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`rxftlr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@rxftlr`] module"]
#[doc(alias = "RXFTLR")]
pub type Rxftlr = crate::Reg<rxftlr::RxftlrSpec>;
#[doc = "Receive FIFO threshold level"]
pub mod rxftlr;
#[doc = "TXFLR (r) register accessor: Transmit FIFO level register\n\nYou can [`read`](crate::Reg::read) this register and get [`txflr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@txflr`] module"]
#[doc(alias = "TXFLR")]
pub type Txflr = crate::Reg<txflr::TxflrSpec>;
#[doc = "Transmit FIFO level register"]
pub mod txflr;
#[doc = "RXFLR (r) register accessor: Receive FIFO level register\n\nYou can [`read`](crate::Reg::read) this register and get [`rxflr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@rxflr`] module"]
#[doc(alias = "RXFLR")]
pub type Rxflr = crate::Reg<rxflr::RxflrSpec>;
#[doc = "Receive FIFO level register"]
pub mod rxflr;
#[doc = "SR (r) register accessor: Status register\n\nYou can [`read`](crate::Reg::read) this register and get [`sr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@sr`] module"]
#[doc(alias = "SR")]
pub type Sr = crate::Reg<sr::SrSpec>;
#[doc = "Status register"]
pub mod sr;
#[doc = "IER (rw) register accessor: Interrupt enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`ier::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ier::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@ier`] module"]
#[doc(alias = "IER")]
pub type Ier = crate::Reg<ier::IerSpec>;
#[doc = "Interrupt enable register"]
pub mod ier;
#[doc = "ISR (r) register accessor: Interrupt status register\n\nYou can [`read`](crate::Reg::read) this register and get [`isr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@isr`] module"]
#[doc(alias = "ISR")]
pub type Isr = crate::Reg<isr::IsrSpec>;
#[doc = "Interrupt status register"]
pub mod isr;
#[doc = "RISR (r) register accessor: Raw interrupt status register\n\nYou can [`read`](crate::Reg::read) this register and get [`risr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@risr`] module"]
#[doc(alias = "RISR")]
pub type Risr = crate::Reg<risr::RisrSpec>;
#[doc = "Raw interrupt status register"]
pub mod risr;
#[doc = "TXOICR (r) register accessor: Transmit FIFO overflow interrupt clear register\n\nYou can [`read`](crate::Reg::read) this register and get [`txoicr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@txoicr`] module"]
#[doc(alias = "TXOICR")]
pub type Txoicr = crate::Reg<txoicr::TxoicrSpec>;
#[doc = "Transmit FIFO overflow interrupt clear register"]
pub mod txoicr;
#[doc = "RXOICR (r) register accessor: Receive FIFO overflow interrupt clear register\n\nYou can [`read`](crate::Reg::read) this register and get [`rxoicr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@rxoicr`] module"]
#[doc(alias = "RXOICR")]
pub type Rxoicr = crate::Reg<rxoicr::RxoicrSpec>;
#[doc = "Receive FIFO overflow interrupt clear register"]
pub mod rxoicr;
#[doc = "RXUICR (r) register accessor: Receive FIFO underflow interrupt clear register\n\nYou can [`read`](crate::Reg::read) this register and get [`rxuicr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@rxuicr`] module"]
#[doc(alias = "RXUICR")]
pub type Rxuicr = crate::Reg<rxuicr::RxuicrSpec>;
#[doc = "Receive FIFO underflow interrupt clear register"]
pub mod rxuicr;
#[doc = "MSTICR (r) register accessor: Multi-master interrupt clear register\n\nYou can [`read`](crate::Reg::read) this register and get [`msticr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@msticr`] module"]
#[doc(alias = "MSTICR")]
pub type Msticr = crate::Reg<msticr::MsticrSpec>;
#[doc = "Multi-master interrupt clear register"]
pub mod msticr;
#[doc = "ICR (r) register accessor: Interrupt clear register\n\nYou can [`read`](crate::Reg::read) this register and get [`icr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@icr`] module"]
#[doc(alias = "ICR")]
pub type Icr = crate::Reg<icr::IcrSpec>;
#[doc = "Interrupt clear register"]
pub mod icr;
#[doc = "DMACR (rw) register accessor: DMA control register\n\nYou can [`read`](crate::Reg::read) this register and get [`dmacr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dmacr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@dmacr`] module"]
#[doc(alias = "DMACR")]
pub type Dmacr = crate::Reg<dmacr::DmacrSpec>;
#[doc = "DMA control register"]
pub mod dmacr;
#[doc = "DMATDLR (rw) register accessor: DMA transmit data level\n\nYou can [`read`](crate::Reg::read) this register and get [`dmatdlr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dmatdlr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@dmatdlr`] module"]
#[doc(alias = "DMATDLR")]
pub type Dmatdlr = crate::Reg<dmatdlr::DmatdlrSpec>;
#[doc = "DMA transmit data level"]
pub mod dmatdlr;
#[doc = "DMARDLR (rw) register accessor: DMA receive data level\n\nYou can [`read`](crate::Reg::read) this register and get [`dmardlr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dmardlr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@dmardlr`] module"]
#[doc(alias = "DMARDLR")]
pub type Dmardlr = crate::Reg<dmardlr::DmardlrSpec>;
#[doc = "DMA receive data level"]
pub mod dmardlr;
#[doc = "DR (rw) register accessor: Data register\n\nYou can [`read`](crate::Reg::read) this register and get [`dr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@dr`] module"]
#[doc(alias = "DR")]
pub type Dr = crate::Reg<dr::DrSpec>;
#[doc = "Data register"]
pub mod dr;
#[doc = "RX_SAMPLE_DLY (rw) register accessor: RX sample delay register\n\nYou can [`read`](crate::Reg::read) this register and get [`rx_sample_dly::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`rx_sample_dly::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@rx_sample_dly`] module"]
#[doc(alias = "RX_SAMPLE_DLY")]
pub type RxSampleDly = crate::Reg<rx_sample_dly::RxSampleDlySpec>;
#[doc = "RX sample delay register"]
pub mod rx_sample_dly;
#[doc = "ESPICR (rw) register accessor: Enhanced SPI control register\n\nYou can [`read`](crate::Reg::read) this register and get [`espicr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`espicr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@espicr`] module"]
#[doc(alias = "ESPICR")]
pub type Espicr = crate::Reg<espicr::EspicrSpec>;
#[doc = "Enhanced SPI control register"]
pub mod espicr;
