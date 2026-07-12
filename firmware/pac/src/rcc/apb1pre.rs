#[doc = "Register `APB1PRE` reader"]
pub type R = crate::R<Apb1preSpec>;
#[doc = "Register `APB1PRE` writer"]
pub type W = crate::W<Apb1preSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "APB1 prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`apb1pre::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`apb1pre::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Apb1preSpec;
impl crate::RegisterSpec for Apb1preSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`apb1pre::R`](R) reader structure"]
impl crate::Readable for Apb1preSpec {}
#[doc = "`write(|w| ..)` method takes [`apb1pre::W`](W) writer structure"]
impl crate::Writable for Apb1preSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets APB1PRE to value 0"]
impl crate::Resettable for Apb1preSpec {}
