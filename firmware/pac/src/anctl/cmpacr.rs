#[doc = "Register `CMPACR` reader"]
pub type R = crate::R<CmpacrSpec>;
#[doc = "Register `CMPACR` writer"]
pub type W = crate::W<CmpacrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Comparator A control register\n\nYou can [`read`](crate::Reg::read) this register and get [`cmpacr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`cmpacr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct CmpacrSpec;
impl crate::RegisterSpec for CmpacrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`cmpacr::R`](R) reader structure"]
impl crate::Readable for CmpacrSpec {}
#[doc = "`write(|w| ..)` method takes [`cmpacr::W`](W) writer structure"]
impl crate::Writable for CmpacrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets CMPACR to value 0"]
impl crate::Resettable for CmpacrSpec {}
