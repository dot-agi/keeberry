#[doc = "Register `BDRSTR` reader"]
pub type R = crate::R<BdrstrSpec>;
#[doc = "Register `BDRSTR` writer"]
pub type W = crate::W<BdrstrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Battery domain reset register\n\nYou can [`read`](crate::Reg::read) this register and get [`bdrstr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`bdrstr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct BdrstrSpec;
impl crate::RegisterSpec for BdrstrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`bdrstr::R`](R) reader structure"]
impl crate::Readable for BdrstrSpec {}
#[doc = "`write(|w| ..)` method takes [`bdrstr::W`](W) writer structure"]
impl crate::Writable for BdrstrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets BDRSTR to value 0"]
impl crate::Resettable for BdrstrSpec {}
