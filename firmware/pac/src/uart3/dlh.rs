#[doc = "Register `DLH` reader"]
pub type R = crate::R<DlhSpec>;
#[doc = "Register `DLH` writer"]
pub type W = crate::W<DlhSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Divisor latch (high)\n\nYou can [`read`](crate::Reg::read) this register and get [`dlh::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dlh::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct DlhSpec;
impl crate::RegisterSpec for DlhSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`dlh::R`](R) reader structure"]
impl crate::Readable for DlhSpec {}
#[doc = "`write(|w| ..)` method takes [`dlh::W`](W) writer structure"]
impl crate::Writable for DlhSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets DLH to value 0"]
impl crate::Resettable for DlhSpec {}
