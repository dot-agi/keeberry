#[doc = "Register `CMPASR` reader"]
pub type R = crate::R<CmpasrSpec>;
#[doc = "Register `CMPASR` writer"]
pub type W = crate::W<CmpasrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Comparator A status register\n\nYou can [`read`](crate::Reg::read) this register and get [`cmpasr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`cmpasr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct CmpasrSpec;
impl crate::RegisterSpec for CmpasrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`cmpasr::R`](R) reader structure"]
impl crate::Readable for CmpasrSpec {}
#[doc = "`write(|w| ..)` method takes [`cmpasr::W`](W) writer structure"]
impl crate::Writable for CmpasrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets CMPASR to value 0"]
impl crate::Resettable for CmpasrSpec {}
