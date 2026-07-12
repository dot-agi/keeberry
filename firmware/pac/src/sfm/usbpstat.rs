#[doc = "Register `USBPSTAT` reader"]
pub type R = crate::R<UsbpstatSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "USB port status register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbpstat::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct UsbpstatSpec;
impl crate::RegisterSpec for UsbpstatSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`usbpstat::R`](R) reader structure"]
impl crate::Readable for UsbpstatSpec {}
#[doc = "`reset()` method sets USBPSTAT to value 0"]
impl crate::Resettable for UsbpstatSpec {}
