#[doc = "Register `GPREG0` reader"]
pub type R = crate::R<Gpreg0Spec>;
#[doc = "Register `GPREG0` writer"]
pub type W = crate::W<Gpreg0Spec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "General-purpose register 0\n\nYou can [`read`](crate::Reg::read) this register and get [`gpreg0::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`gpreg0::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Gpreg0Spec;
impl crate::RegisterSpec for Gpreg0Spec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`gpreg0::R`](R) reader structure"]
impl crate::Readable for Gpreg0Spec {}
#[doc = "`write(|w| ..)` method takes [`gpreg0::W`](W) writer structure"]
impl crate::Writable for Gpreg0Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets GPREG0 to value 0"]
impl crate::Resettable for Gpreg0Spec {}
