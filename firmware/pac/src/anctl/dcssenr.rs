#[doc = "Register `DCSSENR` reader"]
pub type R = crate::R<DcssenrSpec>;
#[doc = "Register `DCSSENR` writer"]
pub type W = crate::W<DcssenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Clock security system enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`dcssenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dcssenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct DcssenrSpec;
impl crate::RegisterSpec for DcssenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`dcssenr::R`](R) reader structure"]
impl crate::Readable for DcssenrSpec {}
#[doc = "`write(|w| ..)` method takes [`dcssenr::W`](W) writer structure"]
impl crate::Writable for DcssenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets DCSSENR to value 0"]
impl crate::Resettable for DcssenrSpec {}
