#[doc = "Register `HSESR` reader"]
pub type R = crate::R<HsesrSpec>;
#[doc = "Register `HSESR` writer"]
pub type W = crate::W<HsesrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "HSE status register\n\nYou can [`read`](crate::Reg::read) this register and get [`hsesr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`hsesr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct HsesrSpec;
impl crate::RegisterSpec for HsesrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`hsesr::R`](R) reader structure"]
impl crate::Readable for HsesrSpec {}
#[doc = "`write(|w| ..)` method takes [`hsesr::W`](W) writer structure"]
impl crate::Writable for HsesrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets HSESR to value 0"]
impl crate::Resettable for HsesrSpec {}
