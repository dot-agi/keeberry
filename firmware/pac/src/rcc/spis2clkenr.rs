#[doc = "Register `SPIS2CLKENR` reader"]
pub type R = crate::R<Spis2clkenrSpec>;
#[doc = "Register `SPIS2CLKENR` writer"]
pub type W = crate::W<Spis2clkenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "SPIS2 clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`spis2clkenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`spis2clkenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Spis2clkenrSpec;
impl crate::RegisterSpec for Spis2clkenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`spis2clkenr::R`](R) reader structure"]
impl crate::Readable for Spis2clkenrSpec {}
#[doc = "`write(|w| ..)` method takes [`spis2clkenr::W`](W) writer structure"]
impl crate::Writable for Spis2clkenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets SPIS2CLKENR to value 0"]
impl crate::Resettable for Spis2clkenrSpec {}
