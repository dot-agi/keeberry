#[doc = "Register `LCKR` reader"]
pub type R = crate::R<LckrSpec>;
#[doc = "Register `LCKR` writer"]
pub type W = crate::W<LckrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Port configuration lock register\n\nYou can [`read`](crate::Reg::read) this register and get [`lckr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`lckr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct LckrSpec;
impl crate::RegisterSpec for LckrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`lckr::R`](R) reader structure"]
impl crate::Readable for LckrSpec {}
#[doc = "`write(|w| ..)` method takes [`lckr::W`](W) writer structure"]
impl crate::Writable for LckrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets LCKR to value 0"]
impl crate::Resettable for LckrSpec {}
