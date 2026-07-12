#[doc = "Register `GPREG1` reader"]
pub type R = crate::R<Gpreg1Spec>;
#[doc = "Register `GPREG1` writer"]
pub type W = crate::W<Gpreg1Spec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "General-purpose register 1\n\nYou can [`read`](crate::Reg::read) this register and get [`gpreg1::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`gpreg1::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Gpreg1Spec;
impl crate::RegisterSpec for Gpreg1Spec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`gpreg1::R`](R) reader structure"]
impl crate::Readable for Gpreg1Spec {}
#[doc = "`write(|w| ..)` method takes [`gpreg1::W`](W) writer structure"]
impl crate::Writable for Gpreg1Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets GPREG1 to value 0"]
impl crate::Resettable for Gpreg1Spec {}
