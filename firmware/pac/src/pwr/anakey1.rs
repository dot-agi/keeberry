#[doc = "Register `ANAKEY1` writer"]
pub type W = crate::W<Anakey1Spec>;
impl core::fmt::Debug for crate::generic::Reg<Anakey1Spec> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "(not readable)")
    }
}
impl W {}
#[doc = "ANCTL write-enable key register 1\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`anakey1::W`](W). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Anakey1Spec;
impl crate::RegisterSpec for Anakey1Spec {
    type Ux = u32;
}
#[doc = "`write(|w| ..)` method takes [`anakey1::W`](W) writer structure"]
impl crate::Writable for Anakey1Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets ANAKEY1 to value 0"]
impl crate::Resettable for Anakey1Spec {}
