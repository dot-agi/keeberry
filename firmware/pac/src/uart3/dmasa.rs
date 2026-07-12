#[doc = "Register `DMASA` writer"]
pub type W = crate::W<DmasaSpec>;
impl core::fmt::Debug for crate::generic::Reg<DmasaSpec> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "(not readable)")
    }
}
impl W {}
#[doc = "DMA software acknowledge\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dmasa::W`](W). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct DmasaSpec;
impl crate::RegisterSpec for DmasaSpec {
    type Ux = u32;
}
#[doc = "`write(|w| ..)` method takes [`dmasa::W`](W) writer structure"]
impl crate::Writable for DmasaSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets DMASA to value 0"]
impl crate::Resettable for DmasaSpec {}
