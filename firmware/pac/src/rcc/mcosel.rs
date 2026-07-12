#[doc = "Register `MCOSEL` reader"]
pub type R = crate::R<McoselSpec>;
#[doc = "Register `MCOSEL` writer"]
pub type W = crate::W<McoselSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Microcontroller clock output select register\n\nYou can [`read`](crate::Reg::read) this register and get [`mcosel::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mcosel::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct McoselSpec;
impl crate::RegisterSpec for McoselSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`mcosel::R`](R) reader structure"]
impl crate::Readable for McoselSpec {}
#[doc = "`write(|w| ..)` method takes [`mcosel::W`](W) writer structure"]
impl crate::Writable for McoselSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets MCOSEL to value 0"]
impl crate::Resettable for McoselSpec {}
