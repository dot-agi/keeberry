#[doc = "Register `DMACR` reader"]
pub type R = crate::R<DmacrSpec>;
#[doc = "Register `DMACR` writer"]
pub type W = crate::W<DmacrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "DMA control register\n\nYou can [`read`](crate::Reg::read) this register and get [`dmacr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dmacr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct DmacrSpec;
impl crate::RegisterSpec for DmacrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`dmacr::R`](R) reader structure"]
impl crate::Readable for DmacrSpec {}
#[doc = "`write(|w| ..)` method takes [`dmacr::W`](W) writer structure"]
impl crate::Writable for DmacrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets DMACR to value 0"]
impl crate::Resettable for DmacrSpec {}
