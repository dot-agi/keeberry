#[doc = "Register `MHSIENR` reader"]
pub type R = crate::R<MhsienrSpec>;
#[doc = "Register `MHSIENR` writer"]
pub type W = crate::W<MhsienrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "MHSI oscillator enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`mhsienr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mhsienr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct MhsienrSpec;
impl crate::RegisterSpec for MhsienrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`mhsienr::R`](R) reader structure"]
impl crate::Readable for MhsienrSpec {}
#[doc = "`write(|w| ..)` method takes [`mhsienr::W`](W) writer structure"]
impl crate::Writable for MhsienrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets MHSIENR to value 0"]
impl crate::Resettable for MhsienrSpec {}
