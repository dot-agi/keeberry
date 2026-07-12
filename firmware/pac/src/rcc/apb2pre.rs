#[doc = "Register `APB2PRE` reader"]
pub type R = crate::R<Apb2preSpec>;
#[doc = "Register `APB2PRE` writer"]
pub type W = crate::W<Apb2preSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "APB2 prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`apb2pre::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`apb2pre::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Apb2preSpec;
impl crate::RegisterSpec for Apb2preSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`apb2pre::R`](R) reader structure"]
impl crate::Readable for Apb2preSpec {}
#[doc = "`write(|w| ..)` method takes [`apb2pre::W`](W) writer structure"]
impl crate::Writable for Apb2preSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets APB2PRE to value 0"]
impl crate::Resettable for Apb2preSpec {}
