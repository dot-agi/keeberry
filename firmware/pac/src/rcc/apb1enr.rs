#[doc = "Register `APB1ENR` reader"]
pub type R = crate::R<Apb1enrSpec>;
#[doc = "Register `APB1ENR` writer"]
pub type W = crate::W<Apb1enrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "APB1 peripheral clock enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`apb1enr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`apb1enr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Apb1enrSpec;
impl crate::RegisterSpec for Apb1enrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`apb1enr::R`](R) reader structure"]
impl crate::Readable for Apb1enrSpec {}
#[doc = "`write(|w| ..)` method takes [`apb1enr::W`](W) writer structure"]
impl crate::Writable for Apb1enrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets APB1ENR to value 0"]
impl crate::Resettable for Apb1enrSpec {}
