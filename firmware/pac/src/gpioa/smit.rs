#[doc = "Register `SMIT` reader"]
pub type R = crate::R<SmitSpec>;
#[doc = "Register `SMIT` writer"]
pub type W = crate::W<SmitSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Port Schmitt-trigger input configuration register (vendor extension)\n\nYou can [`read`](crate::Reg::read) this register and get [`smit::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`smit::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct SmitSpec;
impl crate::RegisterSpec for SmitSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`smit::R`](R) reader structure"]
impl crate::Readable for SmitSpec {}
#[doc = "`write(|w| ..)` method takes [`smit::W`](W) writer structure"]
impl crate::Writable for SmitSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets SMIT to value 0"]
impl crate::Resettable for SmitSpec {}
