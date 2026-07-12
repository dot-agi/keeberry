#[repr(C)]
#[doc = "Register block"]
pub struct RegisterBlock {
    ctrl: Ctrl,
    data: Data,
    dout: [Dout; 8],
    _reserved3: [u8; 0x18],
    usbpcon: Usbpcon,
    usbpsdcsr: Usbpsdcsr,
    usbpstat: Usbpstat,
}
impl RegisterBlock {
    #[doc = "0x00 - Control register"]
    #[inline(always)]
    pub const fn ctrl(&self) -> &Ctrl {
        &self.ctrl
    }
    #[doc = "0x04 - Input data register"]
    #[inline(always)]
    pub const fn data(&self) -> &Data {
        &self.data
    }
    #[doc = "0x08..0x28 - Result register"]
    #[inline(always)]
    pub const fn dout(&self, n: usize) -> &Dout {
        &self.dout[n]
    }
    #[doc = "Iterator for array of:"]
    #[doc = "0x08..0x28 - Result register"]
    #[inline(always)]
    pub fn dout_iter(&self) -> impl Iterator<Item = &Dout> {
        self.dout.iter()
    }
    #[doc = "0x40 - USB port control register"]
    #[inline(always)]
    pub const fn usbpcon(&self) -> &Usbpcon {
        &self.usbpcon
    }
    #[doc = "0x44 - USB port state detect control/status register"]
    #[inline(always)]
    pub const fn usbpsdcsr(&self) -> &Usbpsdcsr {
        &self.usbpsdcsr
    }
    #[doc = "0x48 - USB port status register"]
    #[inline(always)]
    pub const fn usbpstat(&self) -> &Usbpstat {
        &self.usbpstat
    }
}
#[doc = "CTRL (rw) register accessor: Control register\n\nYou can [`read`](crate::Reg::read) this register and get [`ctrl::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ctrl::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@ctrl`] module"]
#[doc(alias = "CTRL")]
pub type Ctrl = crate::Reg<ctrl::CtrlSpec>;
#[doc = "Control register"]
pub mod ctrl;
#[doc = "DATA (rw) register accessor: Input data register\n\nYou can [`read`](crate::Reg::read) this register and get [`data::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`data::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@data`] module"]
#[doc(alias = "DATA")]
pub type Data = crate::Reg<data::DataSpec>;
#[doc = "Input data register"]
pub mod data;
#[doc = "DOUT (r) register accessor: Result register\n\nYou can [`read`](crate::Reg::read) this register and get [`dout::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@dout`] module"]
#[doc(alias = "DOUT")]
pub type Dout = crate::Reg<dout::DoutSpec>;
#[doc = "Result register"]
pub mod dout;
#[doc = "USBPCON (rw) register accessor: USB port control register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbpcon::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`usbpcon::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@usbpcon`] module"]
#[doc(alias = "USBPCON")]
pub type Usbpcon = crate::Reg<usbpcon::UsbpconSpec>;
#[doc = "USB port control register"]
pub mod usbpcon;
#[doc = "USBPSDCSR (rw) register accessor: USB port state detect control/status register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbpsdcsr::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`usbpsdcsr::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@usbpsdcsr`] module"]
#[doc(alias = "USBPSDCSR")]
pub type Usbpsdcsr = crate::Reg<usbpsdcsr::UsbpsdcsrSpec>;
#[doc = "USB port state detect control/status register"]
pub mod usbpsdcsr;
#[doc = "USBPSTAT (r) register accessor: USB port status register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbpstat::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@usbpstat`] module"]
#[doc(alias = "USBPSTAT")]
pub type Usbpstat = crate::Reg<usbpstat::UsbpstatSpec>;
#[doc = "USB port status register"]
pub mod usbpstat;
