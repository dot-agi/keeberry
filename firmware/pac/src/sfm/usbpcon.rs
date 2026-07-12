#[doc = "Register `USBPCON` reader"]
pub type R = crate::R<UsbpconSpec>;
#[doc = "Register `USBPCON` writer"]
pub type W = crate::W<UsbpconSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "USB port control register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbpcon::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`usbpcon::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct UsbpconSpec;
impl crate::RegisterSpec for UsbpconSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`usbpcon::R`](R) reader structure"]
impl crate::Readable for UsbpconSpec {}
#[doc = "`write(|w| ..)` method takes [`usbpcon::W`](W) writer structure"]
impl crate::Writable for UsbpconSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets USBPCON to value 0"]
impl crate::Resettable for UsbpconSpec {}
