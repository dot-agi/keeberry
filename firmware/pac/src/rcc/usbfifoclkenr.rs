#[doc = "Register `USBFIFOCLKENR` reader"]
pub type R = crate::R<UsbfifoclkenrSpec>;
#[doc = "Register `USBFIFOCLKENR` writer"]
pub type W = crate::W<UsbfifoclkenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "USB FIFO clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbfifoclkenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`usbfifoclkenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct UsbfifoclkenrSpec;
impl crate::RegisterSpec for UsbfifoclkenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`usbfifoclkenr::R`](R) reader structure"]
impl crate::Readable for UsbfifoclkenrSpec {}
#[doc = "`write(|w| ..)` method takes [`usbfifoclkenr::W`](W) writer structure"]
impl crate::Writable for UsbfifoclkenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets USBFIFOCLKENR to value 0"]
impl crate::Resettable for UsbfifoclkenrSpec {}
