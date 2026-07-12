#[doc = "Register `RNGCLKENR` reader"]
pub type R = crate::R<RngclkenrSpec>;
#[doc = "Register `RNGCLKENR` writer"]
pub type W = crate::W<RngclkenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "RNG clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`rngclkenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`rngclkenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RngclkenrSpec;
impl crate::RegisterSpec for RngclkenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`rngclkenr::R`](R) reader structure"]
impl crate::Readable for RngclkenrSpec {}
#[doc = "`write(|w| ..)` method takes [`rngclkenr::W`](W) writer structure"]
impl crate::Writable for RngclkenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets RNGCLKENR to value 0"]
impl crate::Resettable for RngclkenrSpec {}
