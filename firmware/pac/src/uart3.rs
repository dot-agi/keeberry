#[repr(C)]
#[doc = "Register block"]
pub struct RegisterBlock {
    _reserved_0_dll: [u8; 0x04],
    _reserved_1_dlh: [u8; 0x04],
    _reserved_2_fcr: [u8; 0x04],
    lcr: Lcr,
    mcr: Mcr,
    lsr: Lsr,
    msr: Msr,
    scr: Scr,
    _reserved8: [u8; 0x5c],
    usr: Usr,
    tfl: Tfl,
    rfl: Rfl,
    srr: Srr,
    srts: Srts,
    sbcr: Sbcr,
    _reserved14: [u8; 0x04],
    sfe: Sfe,
    srt: Srt,
    stet: Stet,
    htx: Htx,
    dmasa: Dmasa,
    _reserved19: [u8; 0x14],
    dlf: Dlf,
    rar: Rar,
    tar: Tar,
    extlcr: Extlcr,
}
impl RegisterBlock {
    #[doc = "0x00 - Divisor latch (low)"]
    #[inline(always)]
    pub const fn dll(&self) -> &Dll {
        unsafe { &*core::ptr::from_ref(self).cast::<u8>().cast() }
    }
    #[doc = "0x00 - Transmit holding register"]
    #[inline(always)]
    pub const fn thr(&self) -> &Thr {
        unsafe { &*core::ptr::from_ref(self).cast::<u8>().cast() }
    }
    #[doc = "0x00 - Receive buffer register"]
    #[inline(always)]
    pub const fn rbr(&self) -> &Rbr {
        unsafe { &*core::ptr::from_ref(self).cast::<u8>().cast() }
    }
    #[doc = "0x04 - Interrupt enable register"]
    #[inline(always)]
    pub const fn ier(&self) -> &Ier {
        unsafe { &*core::ptr::from_ref(self).cast::<u8>().add(4).cast() }
    }
    #[doc = "0x04 - Divisor latch (high)"]
    #[inline(always)]
    pub const fn dlh(&self) -> &Dlh {
        unsafe { &*core::ptr::from_ref(self).cast::<u8>().add(4).cast() }
    }
    #[doc = "0x08 - FIFO control register"]
    #[inline(always)]
    pub const fn fcr(&self) -> &Fcr {
        unsafe { &*core::ptr::from_ref(self).cast::<u8>().add(8).cast() }
    }
    #[doc = "0x08 - Interrupt identification register"]
    #[inline(always)]
    pub const fn iir(&self) -> &Iir {
        unsafe { &*core::ptr::from_ref(self).cast::<u8>().add(8).cast() }
    }
    #[doc = "0x0c - Line control register"]
    #[inline(always)]
    pub const fn lcr(&self) -> &Lcr {
        &self.lcr
    }
    #[doc = "0x10 - Modem control register"]
    #[inline(always)]
    pub const fn mcr(&self) -> &Mcr {
        &self.mcr
    }
    #[doc = "0x14 - Line status register"]
    #[inline(always)]
    pub const fn lsr(&self) -> &Lsr {
        &self.lsr
    }
    #[doc = "0x18 - Modem status register"]
    #[inline(always)]
    pub const fn msr(&self) -> &Msr {
        &self.msr
    }
    #[doc = "0x1c - Scratchpad register"]
    #[inline(always)]
    pub const fn scr(&self) -> &Scr {
        &self.scr
    }
    #[doc = "0x7c - UART status register"]
    #[inline(always)]
    pub const fn usr(&self) -> &Usr {
        &self.usr
    }
    #[doc = "0x80 - Transmit FIFO level"]
    #[inline(always)]
    pub const fn tfl(&self) -> &Tfl {
        &self.tfl
    }
    #[doc = "0x84 - Receive FIFO level"]
    #[inline(always)]
    pub const fn rfl(&self) -> &Rfl {
        &self.rfl
    }
    #[doc = "0x88 - Software reset register"]
    #[inline(always)]
    pub const fn srr(&self) -> &Srr {
        &self.srr
    }
    #[doc = "0x8c - Shadow request to send"]
    #[inline(always)]
    pub const fn srts(&self) -> &Srts {
        &self.srts
    }
    #[doc = "0x90 - Shadow break control register"]
    #[inline(always)]
    pub const fn sbcr(&self) -> &Sbcr {
        &self.sbcr
    }
    #[doc = "0x98 - Shadow FIFO enable"]
    #[inline(always)]
    pub const fn sfe(&self) -> &Sfe {
        &self.sfe
    }
    #[doc = "0x9c - Shadow RCVR trigger"]
    #[inline(always)]
    pub const fn srt(&self) -> &Srt {
        &self.srt
    }
    #[doc = "0xa0 - Shadow TX empty trigger"]
    #[inline(always)]
    pub const fn stet(&self) -> &Stet {
        &self.stet
    }
    #[doc = "0xa4 - Halt TX"]
    #[inline(always)]
    pub const fn htx(&self) -> &Htx {
        &self.htx
    }
    #[doc = "0xa8 - DMA software acknowledge"]
    #[inline(always)]
    pub const fn dmasa(&self) -> &Dmasa {
        &self.dmasa
    }
    #[doc = "0xc0 - Divisor latch fractional value"]
    #[inline(always)]
    pub const fn dlf(&self) -> &Dlf {
        &self.dlf
    }
    #[doc = "0xc4 - Receive address register"]
    #[inline(always)]
    pub const fn rar(&self) -> &Rar {
        &self.rar
    }
    #[doc = "0xc8 - Transmit address register"]
    #[inline(always)]
    pub const fn tar(&self) -> &Tar {
        &self.tar
    }
    #[doc = "0xcc - Line extended control register"]
    #[inline(always)]
    pub const fn extlcr(&self) -> &Extlcr {
        &self.extlcr
    }
}
#[doc = "RBR (r) register accessor: Receive buffer register\n\nYou can [`read`](crate::Reg::read) this register and get [`rbr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@rbr`] module"]
#[doc(alias = "RBR")]
pub type Rbr = crate::Reg<rbr::RbrSpec>;
#[doc = "Receive buffer register"]
pub mod rbr;
#[doc = "THR (w) register accessor: Transmit holding register\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`thr::W`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@thr`] module"]
#[doc(alias = "THR")]
pub type Thr = crate::Reg<thr::ThrSpec>;
#[doc = "Transmit holding register"]
pub mod thr;
#[doc = "DLL (rw) register accessor: Divisor latch (low)\n\nYou can [`read`](crate::Reg::read) this register and get [`dll::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dll::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@dll`] module"]
#[doc(alias = "DLL")]
pub type Dll = crate::Reg<dll::DllSpec>;
#[doc = "Divisor latch (low)"]
pub mod dll;
#[doc = "DLH (rw) register accessor: Divisor latch (high)\n\nYou can [`read`](crate::Reg::read) this register and get [`dlh::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dlh::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@dlh`] module"]
#[doc(alias = "DLH")]
pub type Dlh = crate::Reg<dlh::DlhSpec>;
#[doc = "Divisor latch (high)"]
pub mod dlh;
#[doc = "IER (rw) register accessor: Interrupt enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`ier::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ier::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@ier`] module"]
#[doc(alias = "IER")]
pub type Ier = crate::Reg<ier::IerSpec>;
#[doc = "Interrupt enable register"]
pub mod ier;
#[doc = "IIR (r) register accessor: Interrupt identification register\n\nYou can [`read`](crate::Reg::read) this register and get [`iir::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@iir`] module"]
#[doc(alias = "IIR")]
pub type Iir = crate::Reg<iir::IirSpec>;
#[doc = "Interrupt identification register"]
pub mod iir;
#[doc = "FCR (w) register accessor: FIFO control register\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`fcr::W`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@fcr`] module"]
#[doc(alias = "FCR")]
pub type Fcr = crate::Reg<fcr::FcrSpec>;
#[doc = "FIFO control register"]
pub mod fcr;
#[doc = "LCR (rw) register accessor: Line control register\n\nYou can [`read`](crate::Reg::read) this register and get [`lcr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`lcr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@lcr`] module"]
#[doc(alias = "LCR")]
pub type Lcr = crate::Reg<lcr::LcrSpec>;
#[doc = "Line control register"]
pub mod lcr;
#[doc = "MCR (rw) register accessor: Modem control register\n\nYou can [`read`](crate::Reg::read) this register and get [`mcr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mcr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@mcr`] module"]
#[doc(alias = "MCR")]
pub type Mcr = crate::Reg<mcr::McrSpec>;
#[doc = "Modem control register"]
pub mod mcr;
#[doc = "LSR (r) register accessor: Line status register\n\nYou can [`read`](crate::Reg::read) this register and get [`lsr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@lsr`] module"]
#[doc(alias = "LSR")]
pub type Lsr = crate::Reg<lsr::LsrSpec>;
#[doc = "Line status register"]
pub mod lsr;
#[doc = "MSR (r) register accessor: Modem status register\n\nYou can [`read`](crate::Reg::read) this register and get [`msr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@msr`] module"]
#[doc(alias = "MSR")]
pub type Msr = crate::Reg<msr::MsrSpec>;
#[doc = "Modem status register"]
pub mod msr;
#[doc = "SCR (rw) register accessor: Scratchpad register\n\nYou can [`read`](crate::Reg::read) this register and get [`scr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`scr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@scr`] module"]
#[doc(alias = "SCR")]
pub type Scr = crate::Reg<scr::ScrSpec>;
#[doc = "Scratchpad register"]
pub mod scr;
#[doc = "USR (r) register accessor: UART status register\n\nYou can [`read`](crate::Reg::read) this register and get [`usr::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@usr`] module"]
#[doc(alias = "USR")]
pub type Usr = crate::Reg<usr::UsrSpec>;
#[doc = "UART status register"]
pub mod usr;
#[doc = "TFL (r) register accessor: Transmit FIFO level\n\nYou can [`read`](crate::Reg::read) this register and get [`tfl::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@tfl`] module"]
#[doc(alias = "TFL")]
pub type Tfl = crate::Reg<tfl::TflSpec>;
#[doc = "Transmit FIFO level"]
pub mod tfl;
#[doc = "RFL (r) register accessor: Receive FIFO level\n\nYou can [`read`](crate::Reg::read) this register and get [`rfl::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@rfl`] module"]
#[doc(alias = "RFL")]
pub type Rfl = crate::Reg<rfl::RflSpec>;
#[doc = "Receive FIFO level"]
pub mod rfl;
#[doc = "SRR (w) register accessor: Software reset register\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`srr::W`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@srr`] module"]
#[doc(alias = "SRR")]
pub type Srr = crate::Reg<srr::SrrSpec>;
#[doc = "Software reset register"]
pub mod srr;
#[doc = "SRTS (rw) register accessor: Shadow request to send\n\nYou can [`read`](crate::Reg::read) this register and get [`srts::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`srts::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@srts`] module"]
#[doc(alias = "SRTS")]
pub type Srts = crate::Reg<srts::SrtsSpec>;
#[doc = "Shadow request to send"]
pub mod srts;
#[doc = "SBCR (rw) register accessor: Shadow break control register\n\nYou can [`read`](crate::Reg::read) this register and get [`sbcr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`sbcr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@sbcr`] module"]
#[doc(alias = "SBCR")]
pub type Sbcr = crate::Reg<sbcr::SbcrSpec>;
#[doc = "Shadow break control register"]
pub mod sbcr;
#[doc = "SFE (rw) register accessor: Shadow FIFO enable\n\nYou can [`read`](crate::Reg::read) this register and get [`sfe::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`sfe::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@sfe`] module"]
#[doc(alias = "SFE")]
pub type Sfe = crate::Reg<sfe::SfeSpec>;
#[doc = "Shadow FIFO enable"]
pub mod sfe;
#[doc = "SRT (rw) register accessor: Shadow RCVR trigger\n\nYou can [`read`](crate::Reg::read) this register and get [`srt::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`srt::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@srt`] module"]
#[doc(alias = "SRT")]
pub type Srt = crate::Reg<srt::SrtSpec>;
#[doc = "Shadow RCVR trigger"]
pub mod srt;
#[doc = "STET (rw) register accessor: Shadow TX empty trigger\n\nYou can [`read`](crate::Reg::read) this register and get [`stet::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`stet::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@stet`] module"]
#[doc(alias = "STET")]
pub type Stet = crate::Reg<stet::StetSpec>;
#[doc = "Shadow TX empty trigger"]
pub mod stet;
#[doc = "HTX (rw) register accessor: Halt TX\n\nYou can [`read`](crate::Reg::read) this register and get [`htx::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`htx::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@htx`] module"]
#[doc(alias = "HTX")]
pub type Htx = crate::Reg<htx::HtxSpec>;
#[doc = "Halt TX"]
pub mod htx;
#[doc = "DMASA (w) register accessor: DMA software acknowledge\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dmasa::W`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@dmasa`] module"]
#[doc(alias = "DMASA")]
pub type Dmasa = crate::Reg<dmasa::DmasaSpec>;
#[doc = "DMA software acknowledge"]
pub mod dmasa;
#[doc = "DLF (rw) register accessor: Divisor latch fractional value\n\nYou can [`read`](crate::Reg::read) this register and get [`dlf::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dlf::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@dlf`] module"]
#[doc(alias = "DLF")]
pub type Dlf = crate::Reg<dlf::DlfSpec>;
#[doc = "Divisor latch fractional value"]
pub mod dlf;
#[doc = "RAR (rw) register accessor: Receive address register\n\nYou can [`read`](crate::Reg::read) this register and get [`rar::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`rar::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@rar`] module"]
#[doc(alias = "RAR")]
pub type Rar = crate::Reg<rar::RarSpec>;
#[doc = "Receive address register"]
pub mod rar;
#[doc = "TAR (rw) register accessor: Transmit address register\n\nYou can [`read`](crate::Reg::read) this register and get [`tar::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`tar::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@tar`] module"]
#[doc(alias = "TAR")]
pub type Tar = crate::Reg<tar::TarSpec>;
#[doc = "Transmit address register"]
pub mod tar;
#[doc = "EXTLCR (rw) register accessor: Line extended control register\n\nYou can [`read`](crate::Reg::read) this register and get [`extlcr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`extlcr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@extlcr`] module"]
#[doc(alias = "EXTLCR")]
pub type Extlcr = crate::Reg<extlcr::ExtlcrSpec>;
#[doc = "Line extended control register"]
pub mod extlcr;
