#[doc = "Register `LSI2RTCENR` reader"]
pub type R = crate::R<Lsi2rtcenrSpec>;
#[doc = "Register `LSI2RTCENR` writer"]
pub type W = crate::W<Lsi2rtcenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "LSI-to-RTC clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`lsi2rtcenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`lsi2rtcenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Lsi2rtcenrSpec;
impl crate::RegisterSpec for Lsi2rtcenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`lsi2rtcenr::R`](R) reader structure"]
impl crate::Readable for Lsi2rtcenrSpec {}
#[doc = "`write(|w| ..)` method takes [`lsi2rtcenr::W`](W) writer structure"]
impl crate::Writable for Lsi2rtcenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets LSI2RTCENR to value 0"]
impl crate::Resettable for Lsi2rtcenrSpec {}
