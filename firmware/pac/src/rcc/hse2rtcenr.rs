#[doc = "Register `HSE2RTCENR` reader"]
pub type R = crate::R<Hse2rtcenrSpec>;
#[doc = "Register `HSE2RTCENR` writer"]
pub type W = crate::W<Hse2rtcenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "HSE-to-RTC clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`hse2rtcenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`hse2rtcenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Hse2rtcenrSpec;
impl crate::RegisterSpec for Hse2rtcenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`hse2rtcenr::R`](R) reader structure"]
impl crate::Readable for Hse2rtcenrSpec {}
#[doc = "`write(|w| ..)` method takes [`hse2rtcenr::W`](W) writer structure"]
impl crate::Writable for Hse2rtcenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets HSE2RTCENR to value 0"]
impl crate::Resettable for Hse2rtcenrSpec {}
