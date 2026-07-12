#[doc = "Register `MHSISR` reader"]
pub type R = crate::R<MhsisrSpec>;
#[doc = "Register `MHSISR` writer"]
pub type W = crate::W<MhsisrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "MHSI oscillator status register\n\nYou can [`read`](crate::Reg::read) this register and get [`mhsisr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mhsisr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct MhsisrSpec;
impl crate::RegisterSpec for MhsisrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`mhsisr::R`](R) reader structure"]
impl crate::Readable for MhsisrSpec {}
#[doc = "`write(|w| ..)` method takes [`mhsisr::W`](W) writer structure"]
impl crate::Writable for MhsisrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets MHSISR to value 0"]
impl crate::Resettable for MhsisrSpec {}
