#[doc = "Register `SPIS1CLKENR` reader"]
pub type R = crate::R<Spis1clkenrSpec>;
#[doc = "Register `SPIS1CLKENR` writer"]
pub type W = crate::W<Spis1clkenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "SPIS1 clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`spis1clkenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`spis1clkenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Spis1clkenrSpec;
impl crate::RegisterSpec for Spis1clkenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`spis1clkenr::R`](R) reader structure"]
impl crate::Readable for Spis1clkenrSpec {}
#[doc = "`write(|w| ..)` method takes [`spis1clkenr::W`](W) writer structure"]
impl crate::Writable for Spis1clkenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets SPIS1CLKENR to value 0"]
impl crate::Resettable for Spis1clkenrSpec {}
