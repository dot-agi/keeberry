#[doc = "Register `LSIENR` reader"]
pub type R = crate::R<LsienrSpec>;
#[doc = "Register `LSIENR` writer"]
pub type W = crate::W<LsienrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "LSI oscillator enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`lsienr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`lsienr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct LsienrSpec;
impl crate::RegisterSpec for LsienrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`lsienr::R`](R) reader structure"]
impl crate::Readable for LsienrSpec {}
#[doc = "`write(|w| ..)` method takes [`lsienr::W`](W) writer structure"]
impl crate::Writable for LsienrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets LSIENR to value 0"]
impl crate::Resettable for LsienrSpec {}
