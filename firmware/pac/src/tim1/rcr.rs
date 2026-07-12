#[doc = "Register `RCR` reader"]
pub type R = crate::R<RcrSpec>;
#[doc = "Register `RCR` writer"]
pub type W = crate::W<RcrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Repetition counter register\n\nYou can [`read`](crate::Reg::read) this register and get [`rcr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`rcr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RcrSpec;
impl crate::RegisterSpec for RcrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`rcr::R`](R) reader structure"]
impl crate::Readable for RcrSpec {}
#[doc = "`write(|w| ..)` method takes [`rcr::W`](W) writer structure"]
impl crate::Writable for RcrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets RCR to value 0"]
impl crate::Resettable for RcrSpec {}
