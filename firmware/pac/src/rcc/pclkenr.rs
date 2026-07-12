#[doc = "Register `PCLKENR` reader"]
pub type R = crate::R<PclkenrSpec>;
#[doc = "Register `PCLKENR` writer"]
pub type W = crate::W<PclkenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Panel PCLK clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`pclkenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pclkenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct PclkenrSpec;
impl crate::RegisterSpec for PclkenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`pclkenr::R`](R) reader structure"]
impl crate::Readable for PclkenrSpec {}
#[doc = "`write(|w| ..)` method takes [`pclkenr::W`](W) writer structure"]
impl crate::Writable for PclkenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets PCLKENR to value 0"]
impl crate::Resettable for PclkenrSpec {}
