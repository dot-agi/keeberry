#[doc = "Register `CMPBSR` reader"]
pub type R = crate::R<CmpbsrSpec>;
#[doc = "Register `CMPBSR` writer"]
pub type W = crate::W<CmpbsrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Comparator B status register\n\nYou can [`read`](crate::Reg::read) this register and get [`cmpbsr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`cmpbsr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct CmpbsrSpec;
impl crate::RegisterSpec for CmpbsrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`cmpbsr::R`](R) reader structure"]
impl crate::Readable for CmpbsrSpec {}
#[doc = "`write(|w| ..)` method takes [`cmpbsr::W`](W) writer structure"]
impl crate::Writable for CmpbsrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets CMPBSR to value 0"]
impl crate::Resettable for CmpbsrSpec {}
