#[doc = "Register `USBPRE` reader"]
pub type R = crate::R<UsbpreSpec>;
#[doc = "Register `USBPRE` writer"]
pub type W = crate::W<UsbpreSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "USB prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`usbpre::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`usbpre::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct UsbpreSpec;
impl crate::RegisterSpec for UsbpreSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`usbpre::R`](R) reader structure"]
impl crate::Readable for UsbpreSpec {}
#[doc = "`write(|w| ..)` method takes [`usbpre::W`](W) writer structure"]
impl crate::Writable for UsbpreSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets USBPRE to value 0"]
impl crate::Resettable for UsbpreSpec {}
