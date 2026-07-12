#[doc = "Register `IWDGCLKENR` reader"]
pub type R = crate::R<IwdgclkenrSpec>;
#[doc = "Register `IWDGCLKENR` writer"]
pub type W = crate::W<IwdgclkenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Independent watchdog clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`iwdgclkenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`iwdgclkenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct IwdgclkenrSpec;
impl crate::RegisterSpec for IwdgclkenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`iwdgclkenr::R`](R) reader structure"]
impl crate::Readable for IwdgclkenrSpec {}
#[doc = "`write(|w| ..)` method takes [`iwdgclkenr::W`](W) writer structure"]
impl crate::Writable for IwdgclkenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets IWDGCLKENR to value 0"]
impl crate::Resettable for IwdgclkenrSpec {}
