#[doc = "Register `USBPSDCSR` reader"]
pub type R = crate::R<UsbpsdcsrSpec>;
#[doc = "Register `USBPSDCSR` writer"]
pub type W = crate::W<UsbpsdcsrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "USB port state detect control/status register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbpsdcsr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`usbpsdcsr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct UsbpsdcsrSpec;
impl crate::RegisterSpec for UsbpsdcsrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`usbpsdcsr::R`](R) reader structure"]
impl crate::Readable for UsbpsdcsrSpec {}
#[doc = "`write(|w| ..)` method takes [`usbpsdcsr::W`](W) writer structure"]
impl crate::Writable for UsbpsdcsrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets USBPSDCSR to value 0"]
impl crate::Resettable for UsbpsdcsrSpec {}
