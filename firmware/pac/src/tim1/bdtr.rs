#[doc = "Register `BDTR` reader"]
pub type R = crate::R<BdtrSpec>;
#[doc = "Register `BDTR` writer"]
pub type W = crate::W<BdtrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Break and dead-time register\n\nYou can [`read`](crate::Reg::read) this register and get [`bdtr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`bdtr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct BdtrSpec;
impl crate::RegisterSpec for BdtrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`bdtr::R`](R) reader structure"]
impl crate::Readable for BdtrSpec {}
#[doc = "`write(|w| ..)` method takes [`bdtr::W`](W) writer structure"]
impl crate::Writable for BdtrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets BDTR to value 0"]
impl crate::Resettable for BdtrSpec {}
