#[doc = "Register `BSRR` writer"]
pub type W = crate::W<BsrrSpec>;
impl core::fmt::Debug for crate::generic::Reg<BsrrSpec> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "(not readable)")
    }
}
impl W {}
#[doc = "Port bit set/reset register\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`bsrr::W`](W). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct BsrrSpec;
impl crate::RegisterSpec for BsrrSpec {
    type Ux = u32;
}
#[doc = "`write(|w| ..)` method takes [`bsrr::W`](W) writer structure"]
impl crate::Writable for BsrrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets BSRR to value 0"]
impl crate::Resettable for BsrrSpec {}
