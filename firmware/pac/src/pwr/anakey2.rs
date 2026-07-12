#[doc = "Register `ANAKEY2` writer"]
pub type W = crate::W<Anakey2Spec>;
impl core::fmt::Debug for crate::generic::Reg<Anakey2Spec> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "(not readable)")
    }
}
impl W {}
#[doc = "ANCTL write-enable key register 2\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`anakey2::W`](W). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Anakey2Spec;
impl crate::RegisterSpec for Anakey2Spec {
    type Ux = u32;
}
#[doc = "`write(|w| ..)` method takes [`anakey2::W`](W) writer structure"]
impl crate::Writable for Anakey2Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets ANAKEY2 to value 0"]
impl crate::Resettable for Anakey2Spec {}
