#[doc = "Register `USBPCR` reader"]
pub type R = crate::R<UsbpcrSpec>;
#[doc = "Register `USBPCR` writer"]
pub type W = crate::W<UsbpcrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "USB PHY control register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbpcr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`usbpcr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct UsbpcrSpec;
impl crate::RegisterSpec for UsbpcrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`usbpcr::R`](R) reader structure"]
impl crate::Readable for UsbpcrSpec {}
#[doc = "`write(|w| ..)` method takes [`usbpcr::W`](W) writer structure"]
impl crate::Writable for UsbpcrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets USBPCR to value 0"]
impl crate::Resettable for UsbpcrSpec {}
