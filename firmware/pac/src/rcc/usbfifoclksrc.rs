#[doc = "Register `USBFIFOCLKSRC` reader"]
pub type R = crate::R<UsbfifoclksrcSpec>;
#[doc = "Register `USBFIFOCLKSRC` writer"]
pub type W = crate::W<UsbfifoclksrcSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "USB FIFO clock source register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbfifoclksrc::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`usbfifoclksrc::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct UsbfifoclksrcSpec;
impl crate::RegisterSpec for UsbfifoclksrcSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`usbfifoclksrc::R`](R) reader structure"]
impl crate::Readable for UsbfifoclksrcSpec {}
#[doc = "`write(|w| ..)` method takes [`usbfifoclksrc::W`](W) writer structure"]
impl crate::Writable for UsbfifoclksrcSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets USBFIFOCLKSRC to value 0"]
impl crate::Resettable for UsbfifoclksrcSpec {}
