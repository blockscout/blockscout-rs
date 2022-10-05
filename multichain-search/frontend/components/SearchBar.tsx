import React, { ChangeEvent, FormEvent } from 'react';


import searchIcon from 'icons/search.svg';
import { Input, InputGroup, InputLeftAddon, InputRightElement } from '@chakra-ui/input';
import { chakra } from '@chakra-ui/react';
import { useColorModeValue } from '@chakra-ui/color-mode';
import { Button } from '@chakra-ui/button';

import styles from '../styles/index.module.css'
// interface Props {
//   onChange: (event: ChangeEvent<HTMLInputElement>) => void;
//   onSubmit: (event: FormEvent<HTMLFormElement>) => void;
// }

export const SearchBar = () => {
  const [ value, setValue ] = React.useState('');

  const handleChange = React.useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setValue(event.target.value);
  }, []);

  const handleSubmit = React.useCallback((event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const url = 'search_results?q' + value;
    window.location.assign(url);
  }, [ value ]);

  
  return (
    <chakra.form noValidate onSubmit={ handleSubmit } display={{ base: 'none', lg: 'block' }} w="100%" className={styles.search}>
    <InputGroup size='md'>
    <InputLeftAddon w="100px" color="#5A349C" fontWeight="bold">Search</InputLeftAddon>
      <Input
        placeholder="by addresses / transactions / block / token... "
        ml="1px"
        onChange={ handleChange }
        borderColor={ useColorModeValue('blackAlpha.100', 'whiteAlpha.200') }
      />

    </InputGroup>
    </chakra.form>

  );
};

