#[doc = "Register `SRTS` reader"]
pub type R = crate::R<SrtsSpec>;
#[doc = "Register `SRTS` writer"]
pub type W = crate::W<SrtsSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Shadow request to send\n\nYou can [`read`](crate::Reg::read) this register and get [`srts::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`srts::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct SrtsSpec;
impl crate::RegisterSpec for SrtsSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`srts::R`](R) reader structure"]
impl crate::Readable for SrtsSpec {}
#[doc = "`write(|w| ..)` method takes [`srts::W`](W) writer structure"]
impl crate::Writable for SrtsSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets SRTS to value 0"]
impl crate::Resettable for SrtsSpec {}
