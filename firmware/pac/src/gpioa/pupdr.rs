#[doc = "Register `PUPDR` reader"]
pub type R = crate::R<PupdrSpec>;
#[doc = "Register `PUPDR` writer"]
pub type W = crate::W<PupdrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Port pull-up/pull-down register\n\nYou can [`read`](crate::Reg::read) this register and get [`pupdr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pupdr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct PupdrSpec;
impl crate::RegisterSpec for PupdrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`pupdr::R`](R) reader structure"]
impl crate::Readable for PupdrSpec {}
#[doc = "`write(|w| ..)` method takes [`pupdr::W`](W) writer structure"]
impl crate::Writable for PupdrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets PUPDR to value 0"]
impl crate::Resettable for PupdrSpec {}
