#[doc = "Register `PLLENR` reader"]
pub type R = crate::R<PllenrSpec>;
#[doc = "Register `PLLENR` writer"]
pub type W = crate::W<PllenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "PLL enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`pllenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pllenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct PllenrSpec;
impl crate::RegisterSpec for PllenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`pllenr::R`](R) reader structure"]
impl crate::Readable for PllenrSpec {}
#[doc = "`write(|w| ..)` method takes [`pllenr::W`](W) writer structure"]
impl crate::Writable for PllenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets PLLENR to value 0"]
impl crate::Resettable for PllenrSpec {}
