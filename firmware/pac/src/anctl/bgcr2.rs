#[doc = "Register `BGCR2` reader"]
pub type R = crate::R<Bgcr2Spec>;
#[doc = "Register `BGCR2` writer"]
pub type W = crate::W<Bgcr2Spec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Bandgap control register 2\n\nYou can [`read`](crate::Reg::read) this register and get [`bgcr2::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`bgcr2::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Bgcr2Spec;
impl crate::RegisterSpec for Bgcr2Spec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`bgcr2::R`](R) reader structure"]
impl crate::Readable for Bgcr2Spec {}
#[doc = "`write(|w| ..)` method takes [`bgcr2::W`](W) writer structure"]
impl crate::Writable for Bgcr2Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets BGCR2 to value 0"]
impl crate::Resettable for Bgcr2Spec {}
