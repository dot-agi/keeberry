#[doc = "Register `LSISR` reader"]
pub type R = crate::R<LsisrSpec>;
#[doc = "Register `LSISR` writer"]
pub type W = crate::W<LsisrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "LSI oscillator status register\n\nYou can [`read`](crate::Reg::read) this register and get [`lsisr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`lsisr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct LsisrSpec;
impl crate::RegisterSpec for LsisrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`lsisr::R`](R) reader structure"]
impl crate::Readable for LsisrSpec {}
#[doc = "`write(|w| ..)` method takes [`lsisr::W`](W) writer structure"]
impl crate::Writable for LsisrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets LSISR to value 0"]
impl crate::Resettable for LsisrSpec {}
