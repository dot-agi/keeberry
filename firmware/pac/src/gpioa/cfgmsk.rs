#[doc = "Register `CFGMSK` reader"]
pub type R = crate::R<CfgmskSpec>;
#[doc = "Register `CFGMSK` writer"]
pub type W = crate::W<CfgmskSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Port per-pin configuration write-mask register (vendor extension)\n\nYou can [`read`](crate::Reg::read) this register and get [`cfgmsk::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`cfgmsk::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct CfgmskSpec;
impl crate::RegisterSpec for CfgmskSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`cfgmsk::R`](R) reader structure"]
impl crate::Readable for CfgmskSpec {}
#[doc = "`write(|w| ..)` method takes [`cfgmsk::W`](W) writer structure"]
impl crate::Writable for CfgmskSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets CFGMSK to value 0"]
impl crate::Resettable for CfgmskSpec {}
