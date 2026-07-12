#[doc = "Register `PVDENR` reader"]
pub type R = crate::R<PvdenrSpec>;
#[doc = "Register `PVDENR` writer"]
pub type W = crate::W<PvdenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Programmable voltage detector enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`pvdenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pvdenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct PvdenrSpec;
impl crate::RegisterSpec for PvdenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`pvdenr::R`](R) reader structure"]
impl crate::Readable for PvdenrSpec {}
#[doc = "`write(|w| ..)` method takes [`pvdenr::W`](W) writer structure"]
impl crate::Writable for PvdenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets PVDENR to value 0"]
impl crate::Resettable for PvdenrSpec {}
