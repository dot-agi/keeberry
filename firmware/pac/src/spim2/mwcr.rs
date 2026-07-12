#[doc = "Register `MWCR` reader"]
pub type R = crate::R<MwcrSpec>;
#[doc = "Register `MWCR` writer"]
pub type W = crate::W<MwcrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Microwire control register\n\nYou can [`read`](crate::Reg::read) this register and get [`mwcr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mwcr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct MwcrSpec;
impl crate::RegisterSpec for MwcrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`mwcr::R`](R) reader structure"]
impl crate::Readable for MwcrSpec {}
#[doc = "`write(|w| ..)` method takes [`mwcr::W`](W) writer structure"]
impl crate::Writable for MwcrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets MWCR to value 0"]
impl crate::Resettable for MwcrSpec {}
